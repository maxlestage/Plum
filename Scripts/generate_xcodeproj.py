#!/usr/bin/env python3
"""Generate Plum.xcodeproj/project.pbxproj from the source tree.

An Xcode project file is a big graph of objects keyed by 96-bit identifiers.
Hand-editing one is how projects end up corrupt, so this script owns it: it
walks the source directories, mirrors them as groups, and derives every
identifier from the object's path with a hash, which keeps the file stable
across regenerations (a re-run produces a byte-identical project unless files
were actually added or removed).

Run it after adding or deleting a source file:

    python3 Scripts/generate_xcodeproj.py
"""

from __future__ import annotations

import hashlib
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROJECT_NAME = "Plum"
APP_TARGET = "Plum"
TEST_TARGET = "PlumTests"
BUNDLE_ID = "app.plum.ios"
DEPLOYMENT_TARGET = "17.0"
SWIFT_VERSION = "5.0"
XCODE_COMPATIBILITY = "Xcode 15.0"
OBJECT_VERSION = 56

SOURCE_EXTENSIONS = {".swift"}
RESOURCE_EXTENSIONS = {".xcassets", ".xcdatamodeld", ".json", ".strings", ".xcstrings"}
# Directories that are a single file reference rather than a group.
BUNDLE_DIRECTORIES = {".xcassets", ".xcdatamodeld", ".lproj"}

FILE_TYPES = {
    ".swift": "sourcecode.swift",
    ".h": "sourcecode.c.h",
    ".m": "sourcecode.c.objc",
    ".xcassets": "folder.assetcatalog",
    ".plist": "text.plist.xml",
    ".json": "text.json",
    ".md": "net.daringfireball.markdown",
    ".entitlements": "text.plist.entitlements",
    ".xcstrings": "text.json.xcstrings",
}

SAFE_TOKEN = re.compile(r"^[A-Za-z0-9_./]+$")


def quoted(value: str) -> str:
    """pbxproj only leaves bare tokens unquoted; everything else needs quotes."""
    if value == "":
        return '""'
    if SAFE_TOKEN.match(value):
        return value
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def identifier(*parts: str) -> str:
    """A stable 24-hex-digit object id derived from what the object is."""
    digest = hashlib.md5("::".join(parts).encode("utf-8")).hexdigest()
    return digest[:24].upper()


class Node:
    """One entry in the project navigator."""

    def __init__(self, name: str, path: Path, is_group: bool):
        self.name = name
        self.path = path
        self.is_group = is_group
        self.children: list[Node] = []

    @property
    def relative(self) -> str:
        return str(self.path.relative_to(ROOT))

    def walk_files(self):
        if not self.is_group:
            yield self
        for child in self.children:
            yield from child.walk_files()


def build_tree(directory: Path) -> Node:
    """Mirrors a directory as a group, sorted so output never churns."""
    node = Node(directory.name, directory, is_group=True)

    entries = sorted(directory.iterdir(), key=lambda p: (p.is_file(), p.name.lower()))
    for entry in entries:
        if entry.name.startswith("."):
            continue
        if entry.is_dir():
            if entry.suffix in BUNDLE_DIRECTORIES:
                node.children.append(Node(entry.name, entry, is_group=False))
            else:
                child = build_tree(entry)
                if child.children:
                    node.children.append(child)
        elif entry.suffix in SOURCE_EXTENSIONS or entry.suffix in RESOURCE_EXTENSIONS:
            node.children.append(Node(entry.name, entry, is_group=False))
    return node


def file_type(path: Path) -> str:
    return FILE_TYPES.get(path.suffix, "text")


def is_source(node: Node) -> bool:
    return node.path.suffix in SOURCE_EXTENSIONS


def is_resource(node: Node) -> bool:
    return node.path.suffix in RESOURCE_EXTENSIONS


class ProjectWriter:
    def __init__(self):
        self.lines: list[str] = []
        self.app_tree = build_tree(ROOT / APP_TARGET)
        self.test_tree = build_tree(ROOT / TEST_TARGET)

        self.app_files = list(self.app_tree.walk_files())
        self.test_files = list(self.test_tree.walk_files())

        # Object ids, all derived from stable strings.
        self.project_id = identifier("project", PROJECT_NAME)
        self.main_group_id = identifier("group", "<root>")
        self.products_group_id = identifier("group", "Products")
        self.app_product_id = identifier("product", APP_TARGET)
        self.test_product_id = identifier("product", TEST_TARGET)

    # -- emitting helpers -------------------------------------------------

    def emit(self, line: str = "") -> None:
        self.lines.append(line)

    def section(self, name: str, body) -> None:
        self.emit(f"/* Begin {name} section */")
        body()
        self.emit(f"/* End {name} section */")
        self.emit()

    # -- sections ---------------------------------------------------------

    def build_file_id(self, node: Node, target: str) -> str:
        return identifier("buildfile", target, node.relative)

    def file_ref_id(self, node: Node) -> str:
        return identifier("fileref", node.relative)

    def group_id(self, node: Node) -> str:
        return identifier("group", node.relative)

    def write_build_files(self) -> None:
        def body():
            for target, files in ((APP_TARGET, self.app_files), (TEST_TARGET, self.test_files)):
                for node in files:
                    phase = "Sources" if is_source(node) else "Resources"
                    self.emit(
                        f"\t\t{self.build_file_id(node, target)} /* {node.name} in {phase} */ = "
                        f"{{isa = PBXBuildFile; fileRef = {self.file_ref_id(node)} /* {node.name} */; }};"
                    )

        self.section("PBXBuildFile", body)

    def write_file_references(self) -> None:
        def body():
            self.emit(
                f"\t\t{self.app_product_id} /* {APP_TARGET}.app */ = {{isa = PBXFileReference; "
                f"explicitFileType = wrapper.application; includeInIndex = 0; "
                f"path = {APP_TARGET}.app; sourceTree = BUILT_PRODUCTS_DIR; }};"
            )
            self.emit(
                f"\t\t{self.test_product_id} /* {TEST_TARGET}.xctest */ = {{isa = PBXFileReference; "
                f"explicitFileType = wrapper.cfbundle; includeInIndex = 0; "
                f"path = {TEST_TARGET}.xctest; sourceTree = BUILT_PRODUCTS_DIR; }};"
            )
            for node in self.app_files + self.test_files:
                self.emit(
                    f"\t\t{self.file_ref_id(node)} /* {node.name} */ = {{isa = PBXFileReference; "
                    f"lastKnownFileType = {file_type(node.path)}; path = {quoted(node.name)}; "
                    f"sourceTree = \"<group>\"; }};"
                )

        self.section("PBXFileReference", body)

    def write_frameworks_phase(self) -> None:
        def body():
            for target in (APP_TARGET, TEST_TARGET):
                self.emit(
                    f"\t\t{identifier('frameworks', target)} /* Frameworks */ = "
                    f"{{isa = PBXFrameworksBuildPhase; buildActionMask = 2147483647; files = ( ); "
                    f"runOnlyForDeploymentPostprocessing = 0; }};"
                )

        self.section("PBXFrameworksBuildPhase", body)

    def write_groups(self) -> None:
        def emit_group(node: Node) -> None:
            children = []
            for child in node.children:
                if child.is_group:
                    children.append((self.group_id(child), child.name))
                else:
                    children.append((self.file_ref_id(child), child.name))

            self.emit(f"\t\t{self.group_id(node)} /* {node.name} */ = {{")
            self.emit("\t\t\tisa = PBXGroup;")
            self.emit("\t\t\tchildren = (")
            for child_id, child_name in children:
                self.emit(f"\t\t\t\t{child_id} /* {child_name} */,")
            self.emit("\t\t\t);")
            self.emit(f"\t\t\tpath = {quoted(node.name)};")
            self.emit('\t\t\tsourceTree = "<group>";')
            self.emit("\t\t};")

            for child in node.children:
                if child.is_group:
                    emit_group(child)

        def body():
            # Root group.
            self.emit(f"\t\t{self.main_group_id} = {{")
            self.emit("\t\t\tisa = PBXGroup;")
            self.emit("\t\t\tchildren = (")
            self.emit(f"\t\t\t\t{self.group_id(self.app_tree)} /* {APP_TARGET} */,")
            self.emit(f"\t\t\t\t{self.group_id(self.test_tree)} /* {TEST_TARGET} */,")
            self.emit(f"\t\t\t\t{self.products_group_id} /* Products */,")
            self.emit("\t\t\t);")
            self.emit('\t\t\tsourceTree = "<group>";')
            self.emit("\t\t};")

            # Products group.
            self.emit(f"\t\t{self.products_group_id} /* Products */ = {{")
            self.emit("\t\t\tisa = PBXGroup;")
            self.emit("\t\t\tchildren = (")
            self.emit(f"\t\t\t\t{self.app_product_id} /* {APP_TARGET}.app */,")
            self.emit(f"\t\t\t\t{self.test_product_id} /* {TEST_TARGET}.xctest */,")
            self.emit("\t\t\t);")
            self.emit("\t\t\tname = Products;")
            self.emit('\t\t\tsourceTree = "<group>";')
            self.emit("\t\t};")

            emit_group(self.app_tree)
            emit_group(self.test_tree)

        self.section("PBXGroup", body)

    def write_native_targets(self) -> None:
        def emit_target(
            name: str,
            product_id: str,
            product_name: str,
            product_type: str,
            dependencies: list[str],
        ) -> None:
            self.emit(f"\t\t{identifier('target', name)} /* {name} */ = {{")
            self.emit("\t\t\tisa = PBXNativeTarget;")
            self.emit(
                f"\t\t\tbuildConfigurationList = {identifier('configlist', 'target', name)} "
                f"/* Build configuration list for PBXNativeTarget \"{name}\" */;"
            )
            self.emit("\t\t\tbuildPhases = (")
            self.emit(f"\t\t\t\t{identifier('sources', name)} /* Sources */,")
            self.emit(f"\t\t\t\t{identifier('frameworks', name)} /* Frameworks */,")
            self.emit(f"\t\t\t\t{identifier('resources', name)} /* Resources */,")
            self.emit("\t\t\t);")
            self.emit("\t\t\tbuildRules = (")
            self.emit("\t\t\t);")
            self.emit("\t\t\tdependencies = (")
            for dependency in dependencies:
                self.emit(f"\t\t\t\t{dependency} /* PBXTargetDependency */,")
            self.emit("\t\t\t);")
            self.emit(f"\t\t\tname = {name};")
            self.emit(f"\t\t\tproductName = {name};")
            self.emit(f"\t\t\tproductReference = {product_id} /* {product_name} */;")
            self.emit(f"\t\t\tproductType = {quoted(product_type)};")
            self.emit("\t\t};")

        def body():
            emit_target(
                APP_TARGET,
                self.app_product_id,
                f"{APP_TARGET}.app",
                "com.apple.product-type.application",
                [],
            )
            emit_target(
                TEST_TARGET,
                self.test_product_id,
                f"{TEST_TARGET}.xctest",
                "com.apple.product-type.bundle.unit-test",
                [identifier("dependency", TEST_TARGET, APP_TARGET)],
            )

        self.section("PBXNativeTarget", body)

    def write_project(self) -> None:
        def body():
            self.emit(f"\t\t{self.project_id} /* Project object */ = {{")
            self.emit("\t\t\tisa = PBXProject;")
            self.emit("\t\t\tattributes = {")
            self.emit("\t\t\t\tBuildIndependentTargetsInParallel = 1;")
            self.emit("\t\t\t\tLastSwiftUpdateCheck = 1500;")
            self.emit("\t\t\t\tLastUpgradeCheck = 1500;")
            self.emit("\t\t\t\tTargetAttributes = {")
            for name in (APP_TARGET, TEST_TARGET):
                self.emit(f"\t\t\t\t\t{identifier('target', name)} = {{")
                self.emit("\t\t\t\t\t\tCreatedOnToolsVersion = 15.0;")
                if name == TEST_TARGET:
                    self.emit(f"\t\t\t\t\t\tTestTargetID = {identifier('target', APP_TARGET)};")
                self.emit("\t\t\t\t\t};")
            self.emit("\t\t\t\t};")
            self.emit("\t\t\t};")
            self.emit(
                f"\t\t\tbuildConfigurationList = {identifier('configlist', 'project', PROJECT_NAME)} "
                f"/* Build configuration list for PBXProject \"{PROJECT_NAME}\" */;"
            )
            self.emit(f"\t\t\tcompatibilityVersion = {quoted(XCODE_COMPATIBILITY)};")
            self.emit("\t\t\tdevelopmentRegion = fr;")
            self.emit("\t\t\thasScannedForEncodings = 0;")
            self.emit("\t\t\tknownRegions = (")
            self.emit("\t\t\t\tfr,")
            self.emit("\t\t\t\ten,")
            self.emit("\t\t\t\tBase,")
            self.emit("\t\t\t);")
            self.emit(f"\t\t\tmainGroup = {self.main_group_id};")
            self.emit(f"\t\t\tproductRefGroup = {self.products_group_id} /* Products */;")
            self.emit('\t\t\tprojectDirPath = "";')
            self.emit('\t\t\tprojectRoot = "";')
            self.emit("\t\t\ttargets = (")
            self.emit(f"\t\t\t\t{identifier('target', APP_TARGET)} /* {APP_TARGET} */,")
            self.emit(f"\t\t\t\t{identifier('target', TEST_TARGET)} /* {TEST_TARGET} */,")
            self.emit("\t\t\t);")
            self.emit("\t\t};")

        self.section("PBXProject", body)

    def write_phase(self, isa: str, label: str, predicate, key: str) -> None:
        def body():
            for target, files in ((APP_TARGET, self.app_files), (TEST_TARGET, self.test_files)):
                self.emit(f"\t\t{identifier(key, target)} /* {label} */ = {{")
                self.emit(f"\t\t\tisa = {isa};")
                self.emit("\t\t\tbuildActionMask = 2147483647;")
                self.emit("\t\t\tfiles = (")
                for node in files:
                    if predicate(node):
                        self.emit(
                            f"\t\t\t\t{self.build_file_id(node, target)} /* {node.name} in {label} */,"
                        )
                self.emit("\t\t\t);")
                self.emit("\t\t\trunOnlyForDeploymentPostprocessing = 0;")
                self.emit("\t\t};")

        self.section(isa, body)

    def write_target_dependency(self) -> None:
        def body():
            proxy_id = identifier("proxy", TEST_TARGET, APP_TARGET)
            self.emit(
                f"\t\t{identifier('dependency', TEST_TARGET, APP_TARGET)} /* PBXTargetDependency */ = {{"
            )
            self.emit("\t\t\tisa = PBXTargetDependency;")
            self.emit(f"\t\t\ttarget = {identifier('target', APP_TARGET)} /* {APP_TARGET} */;")
            self.emit(f"\t\t\ttargetProxy = {proxy_id} /* PBXContainerItemProxy */;")
            self.emit("\t\t};")

        self.section("PBXTargetDependency", body)

    def write_container_proxy(self) -> None:
        def body():
            self.emit(f"\t\t{identifier('proxy', TEST_TARGET, APP_TARGET)} /* PBXContainerItemProxy */ = {{")
            self.emit("\t\t\tisa = PBXContainerItemProxy;")
            self.emit(f"\t\t\tcontainerPortal = {self.project_id} /* Project object */;")
            self.emit("\t\t\tproxyType = 1;")
            self.emit(f"\t\t\tremoteGlobalIDString = {identifier('target', APP_TARGET)};")
            self.emit(f"\t\t\tremoteInfo = {APP_TARGET};")
            self.emit("\t\t};")

        self.section("PBXContainerItemProxy", body)

    # -- build settings ---------------------------------------------------

    def project_settings(self, debug: bool) -> dict[str, str]:
        settings = {
            "ALWAYS_SEARCH_USER_PATHS": "NO",
            "ASSETCATALOG_COMPILER_GENERATE_SWIFT_ASSET_SYMBOL_EXTENSIONS": "YES",
            "CLANG_ANALYZER_NONNULL": "YES",
            "CLANG_ANALYZER_NUMBER_OBJECT_CONVERSION": "YES_AGGRESSIVE",
            "CLANG_ENABLE_MODULES": "YES",
            "CLANG_ENABLE_OBJC_ARC": "YES",
            "CLANG_ENABLE_OBJC_WEAK": "YES",
            "CLANG_WARN_BLOCK_CAPTURE_AUTORELEASING": "YES",
            "CLANG_WARN_BOOL_CONVERSION": "YES",
            "CLANG_WARN_COMMA": "YES",
            "CLANG_WARN_CONSTANT_CONVERSION": "YES",
            "CLANG_WARN_DEPRECATED_OBJC_IMPLEMENTATIONS": "YES",
            "CLANG_WARN_DIRECT_OBJC_ISA_USAGE": "YES_ERROR",
            "CLANG_WARN_DOCUMENTATION_COMMENTS": "YES",
            "CLANG_WARN_EMPTY_BODY": "YES",
            "CLANG_WARN_ENUM_CONVERSION": "YES",
            "CLANG_WARN_INFINITE_RECURSION": "YES",
            "CLANG_WARN_INT_CONVERSION": "YES",
            "CLANG_WARN_NON_LITERAL_NULL_CONVERSION": "YES",
            "CLANG_WARN_OBJC_IMPLICIT_RETAIN_SELF": "YES",
            "CLANG_WARN_OBJC_LITERAL_CONVERSION": "YES",
            "CLANG_WARN_OBJC_ROOT_CLASS": "YES_ERROR",
            "CLANG_WARN_QUOTED_INCLUDE_IN_FRAMEWORK_HEADER": "YES",
            "CLANG_WARN_RANGE_LOOP_ANALYSIS": "YES",
            "CLANG_WARN_STRICT_PROTOTYPES": "YES",
            "CLANG_WARN_SUSPICIOUS_MOVE": "YES",
            "CLANG_WARN_UNGUARDED_AVAILABILITY": "YES_AGGRESSIVE",
            "CLANG_WARN_UNREACHABLE_CODE": "YES",
            "CLANG_WARN__DUPLICATE_METHOD_MATCH": "YES",
            "COPY_PHASE_STRIP": "NO",
            "ENABLE_STRICT_OBJC_MSGSEND": "YES",
            "ENABLE_USER_SCRIPT_SANDBOXING": "YES",
            "GCC_C_LANGUAGE_STANDARD": "gnu17",
            "GCC_NO_COMMON_BLOCKS": "YES",
            "GCC_WARN_64_TO_32_BIT_CONVERSION": "YES",
            "GCC_WARN_ABOUT_RETURN_TYPE": "YES_ERROR",
            "GCC_WARN_UNDECLARED_SELECTOR": "YES",
            "GCC_WARN_UNINITIALIZED_AUTOS": "YES_AGGRESSIVE",
            "GCC_WARN_UNUSED_FUNCTION": "YES",
            "GCC_WARN_UNUSED_VARIABLE": "YES",
            "IPHONEOS_DEPLOYMENT_TARGET": DEPLOYMENT_TARGET,
            "LOCALIZATION_PREFERS_STRING_CATALOGS": "YES",
            "MTL_FAST_MATH": "YES",
            "SDKROOT": "iphoneos",
            "SWIFT_EMIT_LOC_STRINGS": "YES",
        }
        if debug:
            settings.update(
                {
                    "DEBUG_INFORMATION_FORMAT": "dwarf",
                    "ENABLE_TESTABILITY": "YES",
                    "GCC_DYNAMIC_NO_PIC": "NO",
                    "GCC_OPTIMIZATION_LEVEL": "0",
                    "GCC_PREPROCESSOR_DEFINITIONS": '"DEBUG=1 $(inherited)"',
                    "MTL_ENABLE_DEBUG_INFO": "INCLUDE_SOURCE",
                    "ONLY_ACTIVE_ARCH": "YES",
                    "SWIFT_ACTIVE_COMPILATION_CONDITIONS": '"DEBUG $(inherited)"',
                    "SWIFT_OPTIMIZATION_LEVEL": '"-Onone"',
                }
            )
        else:
            settings.update(
                {
                    "DEBUG_INFORMATION_FORMAT": '"dwarf-with-dsym"',
                    "ENABLE_NS_ASSERTIONS": "NO",
                    "MTL_ENABLE_DEBUG_INFO": "NO",
                    "SWIFT_COMPILATION_MODE": "wholemodule",
                    "VALIDATE_PRODUCT": "YES",
                }
            )
        return settings

    def app_settings(self, debug: bool) -> dict[str, str]:
        return {
            "ASSETCATALOG_COMPILER_APPICON_NAME": "AppIcon",
            "ASSETCATALOG_COMPILER_GLOBAL_ACCENT_COLOR_NAME": "AccentColor",
            "CODE_SIGN_STYLE": "Automatic",
            "CURRENT_PROJECT_VERSION": "1",
            "ENABLE_PREVIEWS": "YES",
            "GENERATE_INFOPLIST_FILE": "YES",
            "INFOPLIST_KEY_CFBundleDisplayName": "Plum",
            "INFOPLIST_KEY_LSApplicationCategoryType": '"public.app-category.social-networking"',
            "INFOPLIST_KEY_NSCameraUsageDescription": '"Pour prendre une photo de profil."',
            "INFOPLIST_KEY_NSLocationWhenInUseUsageDescription": '"Pour vous montrer qui est dans le coin."',
            "INFOPLIST_KEY_NSPhotoLibraryUsageDescription": '"Pour choisir vos photos de profil."',
            "INFOPLIST_KEY_UIApplicationSceneManifest_Generation": "YES",
            "INFOPLIST_KEY_UILaunchScreen_Generation": "YES",
            "INFOPLIST_KEY_UISupportedInterfaceOrientations_iPad": (
                '"UIInterfaceOrientationPortrait UIInterfaceOrientationPortraitUpsideDown '
                'UIInterfaceOrientationLandscapeLeft UIInterfaceOrientationLandscapeRight"'
            ),
            "INFOPLIST_KEY_UISupportedInterfaceOrientations_iPhone": '"UIInterfaceOrientationPortrait"',
            "LD_RUNPATH_SEARCH_PATHS": '(\n\t\t\t\t\t"$(inherited)",\n\t\t\t\t\t"@executable_path/Frameworks",\n\t\t\t\t)',
            "MARKETING_VERSION": "1.0",
            "PRODUCT_BUNDLE_IDENTIFIER": BUNDLE_ID,
            "PRODUCT_NAME": '"$(TARGET_NAME)"',
            "SWIFT_VERSION": SWIFT_VERSION,
            "TARGETED_DEVICE_FAMILY": '"1,2"',
        }

    def test_settings(self, debug: bool) -> dict[str, str]:
        return {
            "BUNDLE_LOADER": '"$(TEST_HOST)"',
            "CODE_SIGN_STYLE": "Automatic",
            "CURRENT_PROJECT_VERSION": "1",
            "GENERATE_INFOPLIST_FILE": "YES",
            "MARKETING_VERSION": "1.0",
            "PRODUCT_BUNDLE_IDENTIFIER": f"{BUNDLE_ID}.tests",
            "PRODUCT_NAME": '"$(TARGET_NAME)"',
            "SWIFT_VERSION": SWIFT_VERSION,
            "TARGETED_DEVICE_FAMILY": '"1,2"',
            "TEST_HOST": f'"$(BUILT_PRODUCTS_DIR)/{APP_TARGET}.app/$(BUNDLE_EXECUTABLE_FOLDER_PATH)/{APP_TARGET}"',
        }

    @staticmethod
    def configuration_id(scope: str, owner: str, name: str) -> str:
        """Scoped because the project and the app target share a name."""
        return identifier("config", scope, owner, name)

    def write_build_configurations(self) -> None:
        def emit_configuration(scope: str, owner: str, name: str, settings: dict[str, str]) -> None:
            self.emit(f"\t\t{self.configuration_id(scope, owner, name)} /* {name} */ = {{")
            self.emit("\t\t\tisa = XCBuildConfiguration;")
            self.emit("\t\t\tbuildSettings = {")
            for key in sorted(settings):
                self.emit(f"\t\t\t\t{key} = {settings[key]};")
            self.emit("\t\t\t};")
            self.emit(f"\t\t\tname = {name};")
            self.emit("\t\t};")

        def body():
            emit_configuration("project", PROJECT_NAME, "Debug", self.project_settings(debug=True))
            emit_configuration("project", PROJECT_NAME, "Release", self.project_settings(debug=False))
            emit_configuration("target", APP_TARGET, "Debug", self.app_settings(debug=True))
            emit_configuration("target", APP_TARGET, "Release", self.app_settings(debug=False))
            emit_configuration("target", TEST_TARGET, "Debug", self.test_settings(debug=True))
            emit_configuration("target", TEST_TARGET, "Release", self.test_settings(debug=False))

        self.section("XCBuildConfiguration", body)

    def write_configuration_lists(self) -> None:
        def emit_list(list_id: str, comment: str, scope: str, owner: str) -> None:
            self.emit(f"\t\t{list_id} /* {comment} */ = {{")
            self.emit("\t\t\tisa = XCConfigurationList;")
            self.emit("\t\t\tbuildConfigurations = (")
            self.emit(f"\t\t\t\t{self.configuration_id(scope, owner, 'Debug')} /* Debug */,")
            self.emit(f"\t\t\t\t{self.configuration_id(scope, owner, 'Release')} /* Release */,")
            self.emit("\t\t\t);")
            self.emit("\t\t\tdefaultConfigurationIsVisible = 0;")
            self.emit("\t\t\tdefaultConfigurationName = Release;")
            self.emit("\t\t};")

        def body():
            emit_list(
                identifier("configlist", "project", PROJECT_NAME),
                f'Build configuration list for PBXProject "{PROJECT_NAME}"',
                "project",
                PROJECT_NAME,
            )
            for target in (APP_TARGET, TEST_TARGET):
                emit_list(
                    identifier("configlist", "target", target),
                    f'Build configuration list for PBXNativeTarget "{target}"',
                    "target",
                    target,
                )

        self.section("XCConfigurationList", body)

    # -- top level --------------------------------------------------------

    def render(self) -> str:
        self.emit("// !$*UTF8*$!")
        self.emit("{")
        self.emit("\tarchiveVersion = 1;")
        self.emit("\tclasses = {")
        self.emit("\t};")
        self.emit(f"\tobjectVersion = {OBJECT_VERSION};")
        self.emit("\tobjects = {")
        self.emit()

        self.write_build_files()
        self.write_container_proxy()
        self.write_file_references()
        self.write_frameworks_phase()
        self.write_groups()
        self.write_native_targets()
        self.write_project()
        self.write_phase("PBXResourcesBuildPhase", "Resources", is_resource, "resources")
        self.write_phase("PBXSourcesBuildPhase", "Sources", is_source, "sources")
        self.write_target_dependency()
        self.write_build_configurations()
        self.write_configuration_lists()

        self.lines[-1:] = []  # drop the trailing blank line before the closing brace
        self.emit("\t};")
        self.emit(f"\trootObject = {self.project_id} /* Project object */;")
        self.emit("}")
        return "\n".join(self.lines) + "\n"


SCHEME_TEMPLATE = """<?xml version="1.0" encoding="UTF-8"?>
<Scheme
   LastUpgradeVersion = "1500"
   version = "1.7">
   <BuildAction
      parallelizeBuildables = "YES"
      buildImplicitDependencies = "YES">
      <BuildActionEntries>
         <BuildActionEntry
            buildForTesting = "YES"
            buildForRunning = "YES"
            buildForProfiling = "YES"
            buildForArchiving = "YES"
            buildForAnalyzing = "YES">
            <BuildableReference
               BuildableIdentifier = "primary"
               BlueprintIdentifier = "{app_target_id}"
               BuildableName = "{app_target}.app"
               BlueprintName = "{app_target}"
               ReferencedContainer = "container:{project}.xcodeproj">
            </BuildableReference>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <TestAction
      buildConfiguration = "Debug"
      selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB"
      selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB"
      shouldUseLaunchSchemeArgsEnv = "YES">
      <Testables>
         <TestableReference
            skipped = "NO">
            <BuildableReference
               BuildableIdentifier = "primary"
               BlueprintIdentifier = "{test_target_id}"
               BuildableName = "{test_target}.xctest"
               BlueprintName = "{test_target}"
               ReferencedContainer = "container:{project}.xcodeproj">
            </BuildableReference>
         </TestableReference>
      </Testables>
   </TestAction>
   <LaunchAction
      buildConfiguration = "Debug"
      selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB"
      selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB"
      launchStyle = "0"
      useCustomWorkingDirectory = "NO"
      ignoresPersistentStateOnLaunch = "NO"
      debugDocumentVersioning = "YES"
      debugServiceExtension = "internal"
      allowLocationSimulation = "YES">
      <EnvironmentVariables>
         <EnvironmentVariable
            key = "PLUM_DEMO_MODE"
            value = "1"
            isEnabled = "YES">
         </EnvironmentVariable>
         <EnvironmentVariable
            key = "PLUM_API_BASE_URL"
            value = "http://127.0.0.1:8080/api/v1"
            isEnabled = "NO">
         </EnvironmentVariable>
         <EnvironmentVariable
            key = "PLUM_WS_BASE_URL"
            value = "ws://127.0.0.1:8080/ws"
            isEnabled = "NO">
         </EnvironmentVariable>
      </EnvironmentVariables>
      <BuildableProductRunnable
         runnableDebuggingMode = "0">
         <BuildableReference
            BuildableIdentifier = "primary"
            BlueprintIdentifier = "{app_target_id}"
            BuildableName = "{app_target}.app"
            BlueprintName = "{app_target}"
            ReferencedContainer = "container:{project}.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </LaunchAction>
   <ProfileAction
      buildConfiguration = "Release"
      shouldUseLaunchSchemeArgsEnv = "YES"
      savedToolIdentifier = ""
      useCustomWorkingDirectory = "NO"
      debugDocumentVersioning = "YES">
      <BuildableProductRunnable
         runnableDebuggingMode = "0">
         <BuildableReference
            BuildableIdentifier = "primary"
            BlueprintIdentifier = "{app_target_id}"
            BuildableName = "{app_target}.app"
            BlueprintName = "{app_target}"
            ReferencedContainer = "container:{project}.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </ProfileAction>
   <AnalyzeAction
      buildConfiguration = "Debug">
   </AnalyzeAction>
   <ArchiveAction
      buildConfiguration = "Release"
      revealArchiveInOrganizer = "YES">
   </ArchiveAction>
</Scheme>
"""

WORKSPACE_TEMPLATE = """<?xml version="1.0" encoding="UTF-8"?>
<Workspace
   version = "1.0">
   <FileRef
      location = "self:">
   </FileRef>
</Workspace>
"""


def main() -> None:
    writer = ProjectWriter()
    project_dir = ROOT / f"{PROJECT_NAME}.xcodeproj"
    (project_dir / "project.xcworkspace" / "xcshareddata").mkdir(parents=True, exist_ok=True)
    (project_dir / "xcshareddata" / "xcschemes").mkdir(parents=True, exist_ok=True)

    (project_dir / "project.pbxproj").write_text(writer.render(), encoding="utf-8")
    (project_dir / "project.xcworkspace" / "contents.xcworkspacedata").write_text(
        WORKSPACE_TEMPLATE, encoding="utf-8"
    )
    (project_dir / "xcshareddata" / "xcschemes" / f"{APP_TARGET}.xcscheme").write_text(
        SCHEME_TEMPLATE.format(
            project=PROJECT_NAME,
            app_target=APP_TARGET,
            test_target=TEST_TARGET,
            app_target_id=identifier("target", APP_TARGET),
            test_target_id=identifier("target", TEST_TARGET),
        ),
        encoding="utf-8",
    )

    print(
        f"{PROJECT_NAME}.xcodeproj écrit : "
        f"{len(writer.app_files)} fichiers dans {APP_TARGET}, "
        f"{len(writer.test_files)} dans {TEST_TARGET}."
    )


if __name__ == "__main__":
    main()
