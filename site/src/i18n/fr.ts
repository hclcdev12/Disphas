// Source texts, in French (docs/Oblivio — Identité & Narration de Marque.md,
// docs/manuel_operateur.md). Only what the product really does: every claim
// here must be true of the shipped version. Clear, Purge, Destruct and the
// standard names are never translated.

const fr = {
  languageName: "Français",
  meta: {
    title: "Oblivio — L'amnésie matérielle certifiée",
    description:
      "Effacement sécurisé de disques selon NIST SP 800-88 Rev. 2 et IEEE 2883-2022 : une décision unique, un refus quand l'effacement n'est pas prouvable, un certificat signé vérifiable hors ligne.",
  },
  nav: {
    product: "Produit",
    security: "Sécurité",
    pricing: "Tarifs",
    download: "Télécharger",
    demo: "Démo",
    contact: "Contact",
    menu: "Menu",
    language: "Langue",
  },
  cta: {
    quote: "Demander un devis",
    product: "Découvrir le produit",
    demo: "Essayer la démo",
    contact: "Nous contacter",
  },
  home: {
    eyebrow: "Effacement sécurisé de disques",
    title: "Le droit à l'oubli, gravé au niveau métal.",
    lead: "Oblivio interroge chaque disque au plus bas niveau, applique la méthode d'effacement prévue par NIST SP 800-88 Rev. 2 et IEEE 2883-2022, et scelle le résultat dans un certificat signé, vérifiable hors ligne.",
    soon: "Bientôt disponible : Linux 64 bits, disques SATA et NVMe.",
    certificate: {
      caption: "Certificat d'effacement (exemple fictif)",
      status: "CONFORME",
      rows: [
        ["Disque", "NVMe · S/N FICT0000A001"],
        ["Méthode", "NVMe Sanitize — Block Erase"],
        ["Catégorie", "Purge"],
        ["Validation", "Canaris · 1 000 échantillons"],
        ["Signature", "Ed25519 · vérifiable hors ligne"],
      ],
    },
    steps: {
      title: "Cinq étapes, aucune devinette",
      lead: "L'opérateur confirme ; Oblivio décide à partir de l'état réel du disque.",
      items: [
        { title: "Inspecter", text: "Lecture directe du firmware (IDENTIFY NVMe et ATA) : capacités d'effacement, santé, verrous." },
        { title: "Décider", text: "Une décision unique : méthode et catégorie, réserves, devenir du disque — ou un refus." },
        { title: "Effacer", text: "Commande firmware quand elle existe ; sinon réécriture par l'ordinateur, jamais certifiée Purge." },
        { title: "Vérifier", text: "Contrôle propre à chaque méthode : échantillons comparés avant et après, ou relecture du motif écrit." },
        { title: "Certifier", text: "Attestation signée Ed25519, PDF/A-3 en français, anglais ou allemand, QR code vérifiable." },
      ],
    },
    decision: {
      title: "Une seule décision, prouvée",
      text: "Oblivio ne certifie que ce qu'il peut prouver. Quand un effacement n'est pas vérifiable, il refuse et prescrit la destruction physique.",
      compliant: "CONFORME",
      compliantDetail: "Purge · NVMe Sanitize · validation par échantillons",
      sayNo: "SAY NO",
      sayNoDetail: "Effacement non prouvable · destruction physique requise",
      example: "Exemples d'états affichés par Oblivio",
    },
    pillars: [
      {
        title: "L'amnésie matérielle",
        text: "Injonction directe au contrôleur (NVMe Sanitize, Crypto Erase, ATA Sanitize) pour remettre le composant à son état d'usine, plutôt que d'écraser les secteurs à l'aveugle.",
      },
      {
        title: "L'autorité de décision",
        text: "Politique « Say NO » : effacement invérifiable, refus et destruction prescrite. Disque effacé mais défaillant : effacement certifié, destruction recommandée plutôt que réemploi.",
      },
      {
        title: "La preuve européenne",
        text: "Conçu en Europe. Vos disques et vos certificats ne quittent jamais le poste ; chaque certificat est signé Ed25519 et se vérifie hors ligne avec un vérificateur gratuit.",
      },
    ],
    audiences: {
      title: "Pour ceux qui doivent prouver l'effacement",
      items: [
        { title: "Ateliers ITAD et reconditionneurs", text: "Effacement en lots, un certificat par disque et un certificat de lot pour chaque commande." },
        { title: "Techniciens IT et prestataires", text: "Une décision claire à chaque disque, une preuve signée à remettre au client." },
        { title: "Administrations et grands comptes", text: "Hors ligne par conception, preuve vérifiable par un tiers, sans dépendance au cloud." },
      ],
    },
    band: {
      title: "Préparez votre déploiement",
      text: "Oblivio est en cours de finalisation. Décrivez votre besoin : nous revenons vers vous.",
    },
  },
  product: {
    title: "Le produit",
    lead: "Une application Linux qui inspecte, décide, efface, vérifie et certifie. Elle ne demande qu'une chose à l'opérateur : confirmer.",
    inspection: {
      title: "Inspection au plus bas niveau",
      text: "Oblivio lit les capacités annoncées par le firmware du disque (NVMe Identify, ATA IDENTIFY) par des appels système directs, sans outil tiers dans la chaîne certifiée. Chaque donnée du certificat indique sa provenance.",
    },
    states: {
      title: "Quatre états, quatre consignes",
      items: [
        { tone: "ready", name: "PRÊT", text: "Oblivio peut effacer et prouver l'effacement : la méthode recommandée est proposée." },
        { tone: "neutral", name: "PROTÉGÉ", text: "Disque système, partition montée, swap actif, membre d'un volume : Oblivio n'y touchera pas." },
        { tone: "blocked", name: "BLOQUÉ", text: "Disque sain, mais une action est nécessaire (veille, mot de passe ATA, cycle d'alimentation) : la consigne est affichée." },
        { tone: "sayno", name: "SAY NO", text: "Effacement non prouvable : destruction physique requise, et un rapport de refus signé." },
      ],
    },
    methods: {
      title: "Purge ou Clear, jamais de confusion",
      purge: "Effacement par le firmware du disque (NVMe Sanitize ou Format, ATA Sanitize, ATA Security Erase), qui protège contre une récupération en laboratoire.",
      clear: "Réécriture de chaque secteur par l'ordinateur, avec un flux pseudo-aléatoire vérifié. Une réécriture par l'ordinateur n'est jamais certifiée Purge.",
    },
    validation: {
      title: "Vérifier, puis valider",
      text: "Avant l'effacement, Oblivio pose des échantillons témoins ; après, il les compare. Pour une réécriture, il relit le motif écrit. Sans validation réussie, pas de certificat CONFORME.",
    },
    certificate: {
      title: "Un certificat qui se défend seul",
      items: [
        "Attestation canonique signée Ed25519 : le PDF n'est qu'un rendu, c'est l'attestation qui fait foi.",
        "PDF/A-3 en français, anglais ou allemand, avec l'attestation et le journal détaillé joints.",
        "QR code : l'attestation complète, vérifiable même sur papier.",
        "Trois statuts seulement : CONFORME, NON CONFORME (effacement forcé), REFUS.",
      ],
    },
    batches: {
      title: "Lots pour les gros volumes",
      text: "Ouvrez un lot pour une commande ou un client : chaque disque porte le numéro de lot dans son certificat signé, et la clôture produit un certificat de lot avec les certificats individuels joints.",
    },
    safety: {
      title: "Conçu pour ne jamais se tromper de disque",
      items: [
        "Le disque système et tout disque en cours d'utilisation sont protégés.",
        "Confirmation par les 4 derniers caractères du numéro de série, lus sur l'étiquette.",
        "Le disque est identifié par son identité matérielle, pas par un chemin système.",
        "Reprise après une coupure de courant pendant un effacement firmware.",
      ],
    },
    systems: {
      title: "Systèmes pris en charge",
      deb: "Paquet .deb : Ubuntu 22.04 et plus récent, Debian 12 et plus récent, Linux Mint 21+, Pop!_OS 22.04+.",
      rpm: "Paquet .rpm : Fedora, openSUSE.",
      drives: "Disques SATA et NVMe branchés en direct (carte mère). Les adaptateurs USB ne sont pas pris en charge.",
    },
  },
  security: {
    title: "Sécurité et conformité",
    lead: "La preuve d'effacement n'a de valeur que si elle résiste à l'examen. Oblivio est construit pour cela.",
    standards: {
      title: "Normes de référence",
      text: "Les décisions suivent NIST SP 800-88 Rev. 2 (Guidelines for Media Sanitization) et IEEE 2883-2022 (Standard for Sanitizing Storage). Les termes Clear, Purge et Destruct sont ceux de ces normes.",
      affiliation: "Oblivio n'est affilié ni au NIST ni à l'IEEE et n'est pas certifié par eux ; leurs normes sont citées comme références.",
    },
    offline: {
      title: "Vos données restent chez vous",
      text: "L'effacement, la décision, la signature et l'historique se font sur le poste. Aucune donnée de disque ni aucun certificat n'est envoyé à Oblivio. Aucune télémétrie.",
    },
    chain: {
      title: "Une chaîne de signature vérifiable",
      text: "Chaque poste détient sa propre clé, autorisée par la licence Oblivio ; elle signe une attestation canonique. Un vérificateur n'a besoin que de la clé publique d'Oblivio, sans connexion.",
    },
    verification: {
      title: "Trois niveaux de vérification",
      items: [
        { name: "Papier", text: "Scanner le QR code : l'attestation est authentique et intacte." },
        { name: "PDF", text: "Vérifier le fichier : en plus, le journal joint correspond à l'attestation." },
        { name: "Audit", text: "PDF et journal : relecture complète des échantillons et de la chronologie." },
      ],
    },
    privacy: {
      title: "Au service de votre conformité",
      text: "Effacement avant cession ou fin de location, preuve signée de chaque opération, champs opérateur facultatifs et suppression des certificats selon votre durée de conservation.",
    },
    disclosure: {
      title: "Signaler une vulnérabilité",
      text: "Une faille de sécurité dans Oblivio ? Écrivez-nous en privé via la page de contact, en choisissant « Sécurité ». Une adresse dédiée sera publiée avant la commercialisation.",
    },
  },
  pricing: {
    title: "Tarifs",
    lead: "Des licences par poste, pas une facturation au disque. Les tarifs seront publiés prochainement.",
    notice: "Tarifs à venir",
    plans: [
      { name: "Diagnostic", audience: "Pour évaluer", features: ["Inspection des disques", "Décision et recommandation NIST", "Sans effacement"] },
      { name: "Professionnel", audience: "Techniciens IT, indépendants", features: ["Effacements illimités", "Certificats PDF signés", "Historique et vérification"] },
      { name: "Atelier", audience: "ITAD, reconditionneurs, MSP", features: ["Effacements illimités", "Lots et certificats de lot", "Effacements en parallèle"] },
      { name: "Grands comptes", audience: "Administrations, OIV", features: ["Déploiement hors ligne", "Accompagnement", "Sur devis"] },
    ],
  },
  download: {
    title: "Télécharger",
    lead: "Oblivio sera téléchargeable depuis votre espace client, gratuitement pour le diagnostic.",
    soon: "Bientôt disponible",
    account: "Un compte gratuit sera nécessaire : il donne accès aux paquets, à leurs empreintes SHA-256 et aux mises à jour.",
  },
  demo: {
    title: "Démo interactive",
    lead: "La vraie interface d'Oblivio, dans votre navigateur, sur des disques fictifs. Rien n'est installé, aucun disque n'est touché.",
    launch: "Lancer la démo",
    notice: "Disques fictifs · durées accélérées · certificats signés avec une clé de test publique, donc non authentiques.",
    desktop: "Conçue pour un écran d'ordinateur.",
    tryTitle: "À essayer",
    scenarios: [
      { tone: "ready", label: "PRÊT", drive: "DEMO NVMe Data 2TB", text: "Effacez-le : Purge par NVMe Sanitize, certificat CONFORME." },
      { tone: "ready", label: "PRÊT", drive: "DEMO HDD 4TB", text: "Disque usé : effacement certifié, destruction recommandée plutôt que réemploi." },
      { tone: "blocked", label: "BLOQUÉ", drive: "DEMO SATA SSD 500GB", text: "Gelé (B1) : cliquez sur Actualiser, puis effacez-le. La validation échoue : REFUS." },
      { tone: "blocked", label: "BLOQUÉ", drive: "DEMO SATA SSD Locked", text: "Verrouillé (B2) : déverrouillez-le avec le mot de passe « oblivio »." },
      { tone: "sayno", label: "SAY NO", drive: "DEMO Unresponsive 1TB", text: "Inspection impossible : forcez l'effacement, le certificat sera NON CONFORME." },
      { tone: "sayno", label: "SAY NO", drive: "DEMO NVMe Worn 256GB", text: "Lecture seule : aucun effacement possible, destruction physique requise." },
      { tone: "neutral", label: "PROTÉGÉ", drive: "DEMO NVMe System 1TB", text: "Disque système : Oblivio n'y touche pas." },
    ],
    confirm: "Pour confirmer un effacement, tapez les 4 derniers caractères du numéro de série affiché.",
  },
  contact: {
    title: "Contact et devis",
    lead: "Un projet, une question, un besoin de devis ? Décrivez-le, nous vous répondons par courriel.",
    name: "Nom",
    organization: "Organisation",
    email: "Courriel",
    topic: "Objet",
    topics: { quote: "Demande de devis", question: "Question sur le produit", security: "Sécurité", other: "Autre" },
    workstations: "Nombre de postes envisagés",
    message: "Message",
    send: "Envoyer",
    privacy: "Vos coordonnées servent uniquement à vous répondre.",
    sent: "Message envoyé. Merci, nous vous répondons rapidement.",
    invalid: "Vérifiez les champs : nom, courriel et message sont obligatoires.",
    failed: "L'envoi a échoué. Réessayez un peu plus tard.",
  },
  legal: {
    pending: "Document en préparation : il sera publié avant l'ouverture des ventes.",
    notice: "Mentions légales",
    privacy: "Confidentialité",
    terms: "Conditions de vente",
  },
  footer: {
    madeIn: "Conçu en Europe",
    affiliation: "Oblivio n'est affilié ni au NIST ni à l'IEEE ; leurs normes sont citées comme références.",
  },
};

export default fr;
export type Texts = typeof fr;
