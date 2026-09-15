import type { Copy } from "./types";

export const en: Copy = {
  nav: {
    home: "Home",
    terms: "Terms of use",
    privacy: "Privacy",
    help: "Help and contact",
    languageLabel: "Language",
    skipToContent: "Skip to content",
    themeLabel: "Theme",
    themes: {
      auto: "Automatic theme",
      light: "Light theme",
      dark: "Dark theme",
    },
  },

  hero: {
    headlineTop: "One photo, two sentences.",
    headlineBottom: "Then we'll see.",
    tagline: "Plum is a dating app for iOS.",
    availabilityCta: "When can I get it?",
    questionCta: "Ask a question",
  },

  stepsEyebrow: "How it works",
  steps: [
    {
      title: "Thirty seconds",
      body: "The least it takes for someone to want to reply. A photo is required; the rest takes thirty seconds.",
    },
    {
      title: "You swipe",
      body: "Right for yes, left for no, up if you really mean it. The chevron opens the full profile when three lines aren't enough to decide.",
    },
    {
      title: "You talk, or you don't",
      body: "A match opens a conversation. Nobody has to use it, and leaving takes two taps.",
    },
  ],

  principlesEyebrow: "What we decided",
  principles: [
    {
      title: "No mysterious algorithm",
      body: "The deck sorts by distance and by your filters. That's all. Nothing is sold, and nobody is promoted for paying.",
    },
    {
      title: "Blocking closes the conversation",
      body: "Not just the deck: the thread disappears on both sides, messages included, and the blocked person can no longer write or make a typing indicator blink. A block that left the conversation open would only be a filter.",
    },
    {
      title: "A report gets read",
      body: "It does not land in a table nobody opens. What gets looked at is how many different people reported someone — one report may be a grudge, several are a pattern.",
    },
    {
      title: "Distances stay vague",
      body: "Under a kilometre, then to the nearest kilometre, then rounded to the nearest five. Enough to know whether it's the same neighbourhood, never enough to find someone.",
    },
    {
      title: "Profiles cannot be harvested",
      body: "One account cannot page through the deck forever to collect names, bios and photos: the number of profiles served per day is capped, far above what a person looks at and far below what a script would want.",
    },
    {
      title: "Usable without seeing the screen",
      body: "The deck is driven by swiping, but every verdict is a VoiceOver action too. An app you can only use with your eyes shuts people out for no reason.",
    },
  ],

  availability: {
    eyebrow: "Availability",
    title: "Not downloadable yet",
    body: "The iOS app is written and tested, and so is the server, but nothing has been through TestFlight yet. Better said here than behind a button that leads nowhere.",
    noMailingList:
      "There's no form to leave your address either: keeping addresses before there's anything to send would be collecting for the sake of it.",
    appLanguageNotice:
      "One more thing worth saying plainly: the app itself is currently in French only. This page is translated; the product is not, yet.",
  },

  footerTagline: "Plum — a dating app. For adults only.",

  draft: {
    heading: "A draft, not a legal document.",
    body: "This text describes accurately what the app does, but no lawyer has read it. One must, before the app goes live: a dating app handles data the GDPR treats as a special category.",
  },

  terms: {
    title: "Terms of use",
    sections: [
      {
        title: "Who can sign up",
        body: "Adults only. Age is asked at sign-up and checked by the server, not only by the app: a client-side check is a courtesy, not a control.",
      },
      {
        title: "What we expect",
        body: "That the profile is yours, that the photos are of you, and that conversations stay bearable. Harassment, impersonation and photos of minors mean deletion without notice.",
      },
      {
        title: "Reporting",
        body: "Any profile can be reported or blocked from a card, from the full profile and from the conversation. Blocking closes the thread on both sides — the conversation and its messages disappear, and the blocked person can no longer send anything. Reports are reviewed.",
      },
      {
        title: "Closing your account",
        body: "Deleting your account happens in the app's settings, without going through us. It takes the matches, the messages and the photos with it.",
      },
    ],
  },

  privacy: {
    title: "Privacy",
    sections: [
      {
        title: "What is collected",
        body: "An email address, a password, a first name, a date of birth, a gender, and whatever you write: bio, city, interests, photos, messages. Plus your location, if you allow it.",
      },
      {
        title: "Location",
        body: "It is used to compute distances, and nothing else. The distances shown are deliberately vague: “under 1 km”, then to the nearest kilometre, then rounded to the nearest five beyond ten. Enough to know whether it's the same neighbourhood, never enough to find someone. Refusing location hides the distances and breaks nothing else.",
      },
      {
        title: "Passwords",
        body: "Stored hashed with Argon2id, never in the clear. The tokens that keep a session open aren't stored either: only their digest is, so that a stolen database hands out no sessions.",
      },
      {
        title: "Special category data",
        body: "Search filters make it possible to infer sexual orientation, which the GDPR lists among the special categories of Article 9. That is exactly what calls for a legal review, and why this page remains a draft.",
      },
      {
        title: "Photos",
        body: "They are re-encoded on arrival, which strips the camera metadata — including the GPS coordinates a phone photo often carries without anyone noticing. A photo's address cannot be guessed, but it is not password-protected either: it opens without signing in, as everywhere photos go through a cache. So anyone who got hold of a link keeps the image, including after an unmatch. Deleting the photo, or the account, removes it from the server.",
      },
      {
        title: "Deletion",
        body: "From the app's settings, at any time, with no request to file.",
      },
    ],
  },

  help: {
    title: "Help and contact",
    sections: [
      {
        title: "Something wrong?",
        body: "There is no contact address published yet, and inventing one that nobody reads would be worse than none. It will appear here before the first TestFlight release.",
      },
      {
        title: "Reporting someone",
        body: "From the app: on the card, the full profile or the conversation. A report goes into a queue that gets reviewed; what matters most is how many different people reported someone. Blocking, on top of reporting, closes the conversation immediately.",
      },
      {
        title: "Service status",
        body: "The API answers on /health. If the app complains about a connection problem, that is the first place to look.",
      },
    ],
  },
};
