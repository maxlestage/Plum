//! Le site de présentation de Plum, en Yew.
//!
//! Il remplace une application React de même forme. Ce qui n'a pas changé, et
//! qui comptait : les treize pages sont pré-rendues en HTML au moment de la
//! construction, donc lisibles sans que rien ne s'exécute — c'est ce que lit
//! un robot, et ce que lit la revue de l'App Store sur les pages légales.
pub mod app;
pub mod components;
pub mod contexte;
pub mod i18n;
pub mod pages;
pub mod theme;
