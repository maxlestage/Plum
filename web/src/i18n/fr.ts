import type { Copy } from "./types";

export const fr: Copy = {
  nav: {
    home: "Accueil",
    terms: "Conditions d'utilisation",
    privacy: "Confidentialité",
    help: "Aide et contact",
    languageLabel: "Langue",
    skipToContent: "Aller au contenu",
    themeLabel: "Thème",
    menuLabel: "Menu",
    themes: {
      auto: "Thème automatique",
      light: "Thème clair",
      dark: "Thème sombre",
    },
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
      title: "Trois profils par jour",
      body: "Pas de pile sans fond, pas de balayage. Trois personnes, la même sélection du matin au soir, et deux boutons : écrire, ou laisser passer. Trois, ça se lit.",
    },
    {
      title: "Écrire, c'est tout le geste",
      body: "Il n'y a pas de « j'aime » qui attend d'être rendu, donc pas d'écran « c'est un match ». Vous écrivez une phrase, elle arrive, et la personne répond ou ne répond pas. Ne pas répondre est une réponse.",
    },
  ],

  principlesEyebrow: "Ce qu'on a décidé",
  principles: [
    {
      title: "Pas d'algorithme mystérieux",
      body: "La sélection du jour trie par distance et par vos critères. C'est tout. Rien n'est vendu, rien n'est mis en avant contre paiement.",
    },
    {
      title: "Bloquer ferme la conversation",
      body: "Pas seulement la sélection : le fil disparaît des deux côtés, les messages avec, et la personne bloquée ne peut plus écrire ni faire clignoter « en train d'écrire ». Un blocage qui laisserait la conversation ouverte ne serait qu'un filtre.",
    },
    {
      title: "Un signalement est lu",
      body: "Il n'atterrit pas dans une table que personne n'ouvre. Ce qui est regardé, c'est le nombre de personnes différentes qui ont signalé quelqu'un — un signalement isolé peut être un dépit, plusieurs sont un motif.",
    },
    {
      title: "Les distances restent vagues",
      body: "Moins d'un kilomètre, puis au kilomètre près, puis arrondies par tranches de cinq. Assez pour savoir si c'est le même quartier, jamais assez pour trouver quelqu'un.",
    },
    {
      title: "Les profils ne s'aspirent pas",
      body: "Il n'y a pas de pile à parcourir pour en récolter les prénoms, les biographies et les photos. Trois profils par jour, c'est aussi ce que voit un script — et on n'écrit qu'aux personnes qu'on vous a proposées.",
    },
    {
      title: "Utilisable sans voir l'écran",
      body: "Des boutons nommés plutôt qu'un geste : « écrire » et « passer » se lisent à voix haute. Une application qu'on ne peut utiliser qu'à l'œil, ou qu'au poignet leste, exclut du monde pour rien.",
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
        body: "Chaque profil peut être signalé ou bloqué depuis une carte, depuis le profil complet et depuis la conversation. Bloquer ferme le fil des deux côtés — la conversation et ses messages disparaissent, et la personne bloquée ne peut plus rien envoyer. Les signalements sont relus.",
      },
      {
        title: "Fin du compte",
        body: "La suppression du compte se fait depuis les réglages de l'application, sans passer par nous. Elle emporte les conversations, les messages et les photos.",
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
        title: "Les photos",
        body: "Elles sont réencodées à la réception, ce qui efface les métadonnées de l'appareil — dont les coordonnées GPS qu'une photo de téléphone porte souvent sans qu'on le sache. L'adresse d'une photo n'est pas devinable, mais elle n'est pas non plus protégée par un mot de passe : elle s'ouvre sans être connecté, comme partout où les photos passent par un cache. Qui a mis la main sur un lien garde donc l'image, y compris après un blocage. Supprimer la photo, ou le compte, la retire du serveur.",
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
        body: "Depuis l'application, sur la carte, le profil complet ou la conversation. Un signalement va dans une file qui est relue ; ce qui compte le plus, c'est le nombre de personnes différentes qui ont signalé quelqu'un. Bloquer, en plus de signaler, referme la conversation immédiatement.",
      },
      {
        title: "État du service",
        body: "L'API répond sur /health. Si l'application se plaint d'un problème de connexion, c'est le premier endroit à regarder.",
      },
    ],
  },
};
