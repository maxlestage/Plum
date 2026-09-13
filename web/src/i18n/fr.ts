import type { Copy } from "./types";

export const fr: Copy = {
  nav: {
    home: "Accueil",
    terms: "Conditions d'utilisation",
    privacy: "Confidentialité",
    help: "Aide et contact",
    languageLabel: "Langue",
    skipToContent: "Aller au contenu",
  },

  hero: {
    headlineTop: "Une photo, deux phrases.",
    headlineBottom: "Et on verra bien.",
    tagline: "Plum est une application de rencontres pour iOS.",
    availabilityCta: "Quand est-ce disponible ?",
    questionCta: "Poser une question",
  },

  stepsEyebrow: "Comment ça marche",
  steps: [
    {
      title: "Trente secondes",
      body: "Le strict minimum pour qu'on ait envie de répondre. Une photo est obligatoire ; le reste se remplit en trente secondes.",
    },
    {
      title: "On balaie",
      body: "À droite si oui, à gauche si non, vers le haut si vraiment. Le chevron ouvre le profil complet quand trois lignes ne suffisent pas à décider.",
    },
    {
      title: "On se parle, ou pas",
      body: "Un match ouvre une conversation. Personne n'est obligé d'y aller, et se retirer tient en deux gestes.",
    },
  ],

  principlesEyebrow: "Ce qu'on a décidé",
  principles: [
    {
      title: "Pas d'algorithme mystérieux",
      body: "Le deck trie par distance et par vos critères. C'est tout. Rien n'est vendu, rien n'est mis en avant contre paiement.",
    },
    {
      title: "Signaler est toujours à un geste",
      body: "Depuis une carte, depuis un profil complet, depuis une conversation. Bloquer retire la personne des deux côtés, immédiatement.",
    },
    {
      title: "Les distances restent vagues",
      body: "Moins d'un kilomètre, puis au kilomètre près, puis arrondies par tranches de cinq. Assez pour savoir si c'est le même quartier, jamais assez pour trouver quelqu'un.",
    },
    {
      title: "Utilisable sans voir l'écran",
      body: "Le deck se pilote au balayage, mais chaque verdict est aussi une action VoiceOver. Une application qu'on ne peut utiliser qu'à l'œil exclut du monde pour rien.",
    },
  ],

  availability: {
    eyebrow: "Disponibilité",
    title: "Pas encore téléchargeable",
    body: "L'application iOS est écrite et testée, le serveur aussi, mais rien n'est encore passé par TestFlight. Autant le dire ici plutôt que de faire patienter devant un bouton qui ne mène nulle part.",
    noMailingList:
      "Il n'y a pas non plus de formulaire pour laisser son adresse : garder des adresses avant d'avoir quoi que ce soit à envoyer serait une collecte sans objet.",
    appLanguageNotice: null,
  },

  footerTagline:
    "Plum — application de rencontres réservée aux personnes majeures.",

  draft: {
    heading: "Brouillon, pas un document juridique.",
    body: "Ce texte décrit fidèlement ce que fait l'application, mais il n'a pas été relu par un juriste. Il doit l'être avant toute mise en service : une application de rencontres traite des données que le RGPD range parmi les catégories particulières.",
  },

  terms: {
    title: "Conditions d'utilisation",
    sections: [
      {
        title: "Qui peut s'inscrire",
        body: "Les personnes majeures uniquement. L'âge est demandé à l'inscription et vérifié par le serveur, pas seulement par l'application : un contrôle côté client est une courtoisie, pas un contrôle.",
      },
      {
        title: "Ce qu'on attend",
        body: "Que le profil soit le vôtre, que les photos soient de vous, et que les conversations restent supportables. Le harcèlement, l'usurpation d'identité et les photos de mineurs entraînent une suppression sans préavis.",
      },
      {
        title: "Signalement",
        body: "Chaque profil peut être signalé ou bloqué depuis une carte, depuis le profil complet et depuis la conversation. Bloquer retire la personne des deux côtés immédiatement.",
      },
      {
        title: "Fin du compte",
        body: "La suppression du compte se fait depuis les réglages de l'application, sans passer par nous. Elle emporte les matchs, les messages et les photos.",
      },
    ],
  },

  privacy: {
    title: "Confidentialité",
    sections: [
      {
        title: "Ce qui est collecté",
        body: "Une adresse email, un mot de passe, un prénom, une date de naissance, un genre, et ce que vous écrivez : bio, ville, centres d'intérêt, photos, messages. Plus la position, si vous l'autorisez.",
      },
      {
        title: "La position",
        body: "Elle sert à calculer des distances, et rien d'autre. Les distances affichées sont volontairement vagues : « moins d'1 km », puis au kilomètre près, puis arrondies par tranches de cinq au-delà de dix. Assez pour savoir si c'est le même quartier, jamais assez pour trouver quelqu'un. Refuser la localisation masque les distances et ne casse rien d'autre.",
      },
      {
        title: "Les mots de passe",
        body: "Stockés hachés en Argon2id, jamais en clair. Les jetons qui gardent une session ouverte ne sont pas stockés non plus : seule leur empreinte l'est, pour qu'une base dérobée ne distribue pas de sessions.",
      },
      {
        title: "Données particulières",
        body: "Les critères de recherche permettent d'inférer une orientation sexuelle, que le RGPD range parmi les catégories particulières de l'article 9. C'est précisément le point qui exige une relecture juridique, et la raison pour laquelle cette page reste un brouillon.",
      },
      {
        title: "Suppression",
        body: "Depuis les réglages de l'application, à tout moment, sans demande à formuler.",
      },
    ],
  },

  help: {
    title: "Aide et contact",
    sections: [
      {
        title: "Un problème ?",
        body: "Il n'y a pas encore d'adresse de contact publiée, et en inventer une qui ne serait pas relevée serait pire que rien. Elle apparaîtra ici avant la première mise à disposition sur TestFlight.",
      },
      {
        title: "Signaler quelqu'un",
        body: "Depuis l'application, sur la carte, le profil complet ou la conversation. C'est plus rapide et mieux tracé que par écrit.",
      },
      {
        title: "État du service",
        body: "L'API répond sur /health. Si l'application se plaint d'un problème de connexion, c'est le premier endroit à regarder.",
      },
    ],
  },
};
