import type { Copy } from "./types";

export const es: Copy = {
  nav: {
    home: "Inicio",
    terms: "Condiciones de uso",
    privacy: "Privacidad",
    help: "Ayuda y contacto",
    languageLabel: "Idioma",
    skipToContent: "Ir al contenido",
  },

  hero: {
    headlineTop: "Una foto, dos frases.",
    headlineBottom: "Y ya veremos.",
    tagline: "Plum es una aplicación de citas para iOS.",
    availabilityCta: "¿Cuándo estará disponible?",
    questionCta: "Hacer una pregunta",
  },

  stepsEyebrow: "Cómo funciona",
  steps: [
    {
      title: "Treinta segundos",
      body: "Lo mínimo para que alguien tenga ganas de responder. La foto es obligatoria; lo demás se rellena en treinta segundos.",
    },
    {
      title: "Se desliza",
      body: "A la derecha si sí, a la izquierda si no, hacia arriba si de verdad. El chevrón abre el perfil completo cuando tres líneas no bastan para decidir.",
    },
    {
      title: "Se habla, o no",
      body: "Un match abre una conversación. Nadie está obligado a usarla, y retirarse son dos gestos.",
    },
  ],

  principlesEyebrow: "Lo que hemos decidido",
  principles: [
    {
      title: "Sin algoritmo misterioso",
      body: "El mazo ordena por distancia y por tus criterios. Nada más. No se vende nada, y nadie aparece antes por pagar.",
    },
    {
      title: "Denunciar está siempre a un gesto",
      body: "Desde una tarjeta, desde un perfil completo, desde una conversación. Bloquear retira a la persona por ambos lados, de inmediato.",
    },
    {
      title: "Las distancias quedan vagas",
      body: "Menos de un kilómetro, luego al kilómetro, luego redondeadas de cinco en cinco. Bastante para saber si es el mismo barrio, nunca bastante para encontrar a alguien.",
    },
    {
      title: "Utilizable sin ver la pantalla",
      body: "El mazo se maneja deslizando, pero cada veredicto es también una acción de VoiceOver. Una aplicación que solo se puede usar con la vista excluye a gente sin motivo.",
    },
  ],

  availability: {
    eyebrow: "Disponibilidad",
    title: "Todavía no se puede descargar",
    body: "La aplicación de iOS está escrita y probada, y el servidor también, pero nada ha pasado aún por TestFlight. Más vale decirlo aquí que hacer esperar ante un botón que no lleva a ninguna parte.",
    noMailingList:
      "Tampoco hay un formulario para dejar el correo: guardar direcciones antes de tener algo que enviar sería recoger datos sin motivo.",
    appLanguageNotice:
      "Una cosa más, dicha claramente: la aplicación está por ahora solo en francés. Esta página está traducida; el producto todavía no.",
  },

  footerTagline: "Plum — aplicación de citas. Solo para mayores de edad.",

  draft: {
    heading: "Borrador, no un documento jurídico.",
    body: "Este texto describe fielmente lo que hace la aplicación, pero no lo ha revisado ningún jurista. Debe hacerlo antes de cualquier puesta en servicio: una aplicación de citas trata datos que el RGPD clasifica entre las categorías especiales.",
  },

  terms: {
    title: "Condiciones de uso",
    sections: [
      {
        title: "Quién puede registrarse",
        body: "Solo personas mayores de edad. La edad se pide al registrarse y la verifica el servidor, no solo la aplicación: una comprobación en el cliente es una cortesía, no un control.",
      },
      {
        title: "Lo que esperamos",
        body: "Que el perfil sea tuyo, que las fotos sean tuyas, y que las conversaciones sigan siendo soportables. El acoso, la suplantación de identidad y las fotos de menores suponen la eliminación sin aviso.",
      },
      {
        title: "Denuncias",
        body: "Cualquier perfil puede denunciarse o bloquearse desde una tarjeta, desde el perfil completo y desde la conversación. Bloquear retira a la persona por ambos lados de inmediato.",
      },
      {
        title: "Cierre de la cuenta",
        body: "La cuenta se elimina desde los ajustes de la aplicación, sin pasar por nosotros. Se lleva consigo los matches, los mensajes y las fotos.",
      },
    ],
  },

  privacy: {
    title: "Privacidad",
    sections: [
      {
        title: "Qué se recoge",
        body: "Un correo electrónico, una contraseña, un nombre, una fecha de nacimiento, un género, y lo que escribas: biografía, ciudad, intereses, fotos, mensajes. Además de la ubicación, si la autorizas.",
      },
      {
        title: "La ubicación",
        body: "Sirve para calcular distancias, y nada más. Las distancias que se muestran son deliberadamente vagas: «menos de 1 km», luego al kilómetro, luego redondeadas de cinco en cinco por encima de diez. Bastante para saber si es el mismo barrio, nunca bastante para encontrar a alguien. Rechazar la ubicación oculta las distancias y no rompe nada más.",
      },
      {
        title: "Las contraseñas",
        body: "Se guardan cifradas con Argon2id, nunca en claro. Los tokens que mantienen abierta una sesión tampoco se guardan: solo su huella, para que una base de datos robada no reparta sesiones.",
      },
      {
        title: "Datos de categoría especial",
        body: "Los criterios de búsqueda permiten inferir la orientación sexual, que el RGPD sitúa entre las categorías especiales del artículo 9. Es justamente el punto que exige una revisión jurídica, y la razón por la que esta página sigue siendo un borrador.",
      },
      {
        title: "Las fotos",
        body: "Se recodifican al llegar, lo que borra los metadatos de la cámara — incluidas las coordenadas GPS que una foto de teléfono suele llevar sin que nadie lo note. La dirección de una foto no se puede adivinar, pero tampoco está protegida por contraseña: se abre sin haber iniciado sesión, como en todas partes donde las fotos pasan por una caché. Quien haya conseguido un enlace conserva por tanto la imagen, incluso después de deshacer un match. Borrar la foto, o la cuenta, la retira del servidor.",
      },
      {
        title: "Eliminación",
        body: "Desde los ajustes de la aplicación, en cualquier momento, sin solicitud que presentar.",
      },
    ],
  },

  help: {
    title: "Ayuda y contacto",
    sections: [
      {
        title: "¿Algún problema?",
        body: "Todavía no hay una dirección de contacto publicada, e inventar una que nadie lea sería peor que nada. Aparecerá aquí antes de la primera distribución por TestFlight.",
      },
      {
        title: "Denunciar a alguien",
        body: "Desde la aplicación, en la tarjeta, el perfil completo o la conversación. Es más rápido y queda mejor registrado que por escrito.",
      },
      {
        title: "Estado del servicio",
        body: "La API responde en /health. Si la aplicación se queja de un problema de conexión, es el primer sitio donde mirar.",
      },
    ],
  },
};
