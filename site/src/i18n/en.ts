// English translation of fr.ts, to be proofread.
import type { Texts } from "./fr";

const en: Texts = {
  languageName: "English",
  meta: {
    title: "Oblivio — Certified hardware amnesia",
    description:
      "Secure drive erasure following NIST SP 800-88 Rev. 2 and IEEE 2883-2022: one decision, a refusal when erasure cannot be proven, a signed certificate that verifies offline.",
  },
  nav: {
    product: "Product",
    security: "Security",
    pricing: "Pricing",
    download: "Download",
    demo: "Demo",
    contact: "Contact",
    account: "Customer area",
    menu: "Menu",
    language: "Language",
  },
  cta: {
    quote: "Request a quote",
    product: "Discover the product",
    demo: "Try the demo",
    contact: "Contact us",
  },
  home: {
    eyebrow: "Secure drive erasure",
    title: "The right to be forgotten, engraved at the metal.",
    lead: "Oblivio questions every drive at the lowest level, applies the erasure method set out by NIST SP 800-88 Rev. 2 and IEEE 2883-2022, and seals the result in a signed certificate that verifies offline.",
    soon: "Coming soon: 64-bit Linux, SATA and NVMe drives.",
    certificate: {
      caption: "Erasure certificate (fictional example)",
      status: "COMPLIANT",
      rows: [
        ["Drive", "NVMe · S/N FICT0000A001"],
        ["Method", "NVMe Sanitize — Block Erase"],
        ["Category", "Purge"],
        ["Validation", "Canaries · 1,000 samples"],
        ["Signature", "Ed25519 · verifies offline"],
      ],
    },
    steps: {
      title: "Five steps, no guessing",
      lead: "The operator confirms; Oblivio decides from the actual state of the drive.",
      items: [
        { title: "Inspect", text: "Direct firmware reads (NVMe and ATA IDENTIFY): erasure capabilities, health, locks." },
        { title: "Decide", text: "One decision: method and category, reservations, what the drive should become — or a refusal." },
        { title: "Erase", text: "A firmware command when there is one; otherwise an overwrite by the computer, never certified Purge." },
        { title: "Verify", text: "A check specific to each method: samples compared before and after, or the written pattern read back." },
        { title: "Certify", text: "Ed25519-signed attestation, PDF/A-3 in English, French or German, verifiable QR code." },
      ],
    },
    decision: {
      title: "One decision, proven",
      text: "Oblivio only certifies what it can prove. When an erasure cannot be verified, it refuses and prescribes physical destruction.",
      compliant: "COMPLIANT",
      compliantDetail: "Purge · NVMe Sanitize · sample validation",
      sayNo: "SAY NO",
      sayNoDetail: "Erasure not provable · physical destruction required",
      example: "Examples of states shown by Oblivio",
    },
    pillars: [
      {
        title: "Hardware amnesia",
        text: "Direct commands to the controller (NVMe Sanitize, Crypto Erase, ATA Sanitize) return the component to its factory state, instead of overwriting sectors blindly.",
      },
      {
        title: "Decision authority",
        text: "“Say NO” policy: erasure that cannot be verified is refused and destruction prescribed. A drive erased but failing: erasure certified, destruction recommended over reuse.",
      },
      {
        title: "European proof",
        text: "Designed in Europe. Your drives and certificates never leave the workstation; every certificate is signed with Ed25519 and verifies offline with a free verifier.",
      },
    ],
    audiences: {
      title: "For those who must prove erasure",
      items: [
        { title: "ITAD and refurbishers", text: "Batch erasure, one certificate per drive and a batch certificate for each order." },
        { title: "IT technicians and service providers", text: "A clear decision for every drive, a signed proof to hand to the customer." },
        { title: "Public sector and large accounts", text: "Offline by design, proof a third party can verify, no cloud dependency." },
      ],
    },
    band: {
      title: "Plan your rollout",
      text: "Oblivio is being finalised. Describe your needs: we will get back to you.",
    },
  },
  product: {
    title: "The product",
    lead: "A Linux application that inspects, decides, erases, verifies and certifies. It asks the operator one thing only: to confirm.",
    inspection: {
      title: "Inspection at the lowest level",
      text: "Oblivio reads the capabilities announced by the drive firmware (NVMe Identify, ATA IDENTIFY) through direct system calls, with no third-party tool in the certified chain. Every value in the certificate states where it comes from.",
    },
    states: {
      title: "Four states, four instructions",
      items: [
        { tone: "ready", name: "READY", text: "Oblivio can erase and prove the erasure: the recommended method is offered." },
        { tone: "neutral", name: "PROTECTED", text: "System drive, mounted partition, active swap, volume member: Oblivio will not touch it." },
        { tone: "blocked", name: "BLOCKED", text: "Healthy drive, but an action is needed (sleep, ATA password, power cycle): the instruction is shown." },
        { tone: "sayno", name: "SAY NO", text: "Erasure not provable: physical destruction required, with a signed refusal report." },
      ],
    },
    methods: {
      title: "Purge or Clear, never confused",
      purge: "Erasure by the drive firmware (NVMe Sanitize or Format, ATA Sanitize, ATA Security Erase), which protects against laboratory recovery.",
      clear: "Every sector overwritten by the computer with a verified pseudo-random stream. An overwrite by the computer is never certified Purge.",
    },
    validation: {
      title: "Verify, then validate",
      text: "Before erasing, Oblivio plants witness samples; afterwards, it compares them. For an overwrite, it reads the written pattern back. Without a successful validation, no COMPLIANT certificate.",
    },
    certificate: {
      title: "A certificate that stands on its own",
      items: [
        "Canonical attestation signed with Ed25519: the PDF is only a rendering, the attestation is what counts.",
        "PDF/A-3 in English, French or German, with the attestation and the detailed log attached.",
        "QR code: the complete attestation, verifiable even on paper.",
        "Three statuses only: COMPLIANT, NON-COMPLIANT (forced erasure), REFUSED.",
      ],
    },
    batches: {
      title: "Batches for large volumes",
      text: "Open a batch for an order or a customer: every drive carries the batch number in its signed certificate, and closing it produces a batch certificate with the individual certificates attached.",
    },
    safety: {
      title: "Built never to pick the wrong drive",
      items: [
        "The system drive and any drive in use are protected.",
        "Confirmation with the last 4 characters of the serial number, read on the label.",
        "The drive is identified by its hardware identity, not by a system path.",
        "Recovery after a power loss during a firmware erasure.",
      ],
    },
    systems: {
      title: "Supported systems",
      deb: ".deb package: Ubuntu 22.04 and later, Debian 12 and later, Linux Mint 21+, Pop!_OS 22.04+.",
      rpm: ".rpm package: Fedora, openSUSE.",
      drives: "SATA and NVMe drives connected directly (motherboard). USB adapters are not supported.",
    },
  },
  security: {
    title: "Security and compliance",
    lead: "Proof of erasure is only worth something if it withstands scrutiny. Oblivio is built for that.",
    standards: {
      title: "Reference standards",
      text: "Decisions follow NIST SP 800-88 Rev. 2 (Guidelines for Media Sanitization) and IEEE 2883-2022 (Standard for Sanitizing Storage). Clear, Purge and Destruct are the terms of these standards.",
      affiliation: "Oblivio is not affiliated with, nor certified by, NIST or IEEE; their standards are cited as references.",
    },
    offline: {
      title: "Your data stays with you",
      text: "Erasure, decision, signature and history all happen on the workstation. No drive data and no certificate is sent to Oblivio. No telemetry.",
    },
    chain: {
      title: "A verifiable signature chain",
      text: "Each workstation holds its own key, authorised by the Oblivio license; it signs a canonical attestation. A verifier only needs Oblivio's public key, with no connection.",
    },
    verification: {
      title: "Three levels of verification",
      items: [
        { name: "Paper", text: "Scan the QR code: the attestation is authentic and intact." },
        { name: "PDF", text: "Check the file: in addition, the attached log matches the attestation." },
        { name: "Audit", text: "PDF and log: complete review of the samples and the timeline." },
      ],
    },
    privacy: {
      title: "Supporting your compliance",
      text: "Erasure before resale or end of lease, signed proof of every operation, optional operator fields, and certificate deletion according to your retention period.",
    },
    disclosure: {
      title: "Report a vulnerability",
      text: "Found a security flaw in Oblivio? Write to us privately through the contact page, choosing “Security”. A dedicated address will be published before sales open.",
    },
  },
  pricing: {
    title: "Pricing",
    lead: "Licenses per workstation, not billing per drive. Prices will be published soon.",
    notice: "Prices coming soon",
    plans: [
      { name: "Diagnostic", audience: "To evaluate", features: ["Drive inspection", "NIST decision and recommendation", "No erasure"] },
      { name: "Professional", audience: "IT technicians, freelancers", features: ["Unlimited erasures", "Signed PDF certificates", "History and verification"] },
      { name: "Workshop", audience: "ITAD, refurbishers, MSPs", features: ["Unlimited erasures", "Batches and batch certificates", "Parallel erasures"] },
      { name: "Large accounts", audience: "Public sector, critical operators", features: ["Offline deployment", "Support", "On quotation"] },
    ],
  },
  download: {
    title: "Download",
    lead: "Oblivio will be downloadable from your customer area, free of charge for diagnostics.",
    soon: "Coming soon",
    account: "A free account will be required: it gives access to the packages, their SHA-256 checksums and updates.",
  },
  demo: {
    title: "Interactive demo",
    lead: "Oblivio's real interface, in your browser, on fictional drives. Nothing is installed, no drive is touched.",
    launch: "Launch the demo",
    notice: "Fictional drives · accelerated durations · certificates signed with a public test key, so not authentic.",
    desktop: "Designed for a computer screen.",
    tryTitle: "Things to try",
    scenarios: [
      { tone: "ready", label: "READY", drive: "DEMO NVMe Data 2TB", text: "Erase it: Purge by NVMe Sanitize, COMPLIANT certificate." },
      { tone: "ready", label: "READY", drive: "DEMO HDD 4TB", text: "Worn drive: erasure certified, destruction recommended over reuse." },
      { tone: "blocked", label: "BLOCKED", drive: "DEMO SATA SSD 500GB", text: "Frozen (B1): click Refresh, then erase it. Validation fails: REFUSED." },
      { tone: "blocked", label: "BLOCKED", drive: "DEMO SATA SSD Locked", text: "Locked (B2): unlock it with the password “oblivio”." },
      { tone: "sayno", label: "SAY NO", drive: "DEMO Unresponsive 1TB", text: "Inspection impossible: force the erasure, the certificate will be NON-COMPLIANT." },
      { tone: "sayno", label: "SAY NO", drive: "DEMO NVMe Worn 256GB", text: "Read-only: no erasure possible, physical destruction required." },
      { tone: "neutral", label: "PROTECTED", drive: "DEMO NVMe System 1TB", text: "System drive: Oblivio does not touch it." },
    ],
    confirm: "To confirm an erasure, type the last 4 characters of the serial number shown.",
  },
  account: {
    title: "Customer area",
    loginTitle: "Sign in",
    loginLead: "Enter your email: we send you a sign-in link, valid for 5 minutes and once only. No password.",
    email: "Email",
    send: "Get the link",
    newAccount: "No account yet? It is created on your first sign-in, free of charge.",
    status: {
      sent: "If the address is valid, a link has just been sent. Check your spam folder.",
      invalid: "Invalid email address.",
      limited: "Too many requests. Try again in an hour.",
      expired: "This link has expired or was already used. Ask for a new one.",
      failed: "Sending failed. Please try again later.",
      out: "You are signed out.",
    },
    signedInAs: "Signed in as",
    signOut: "Sign out",
    downloadsTitle: "Downloads",
    downloadsLead: "Oblivio packages for 64-bit Linux. Check the SHA-256 checksum before installing.",
    noRelease: "No package has been published yet: you will be told when it is out.",
    checksum: "SHA-256",
    licensesTitle: "Licenses",
    licensesLead: "Your subscriptions and workstations will appear here once sales open.",
  },
  contact: {
    title: "Contact and quotes",
    lead: "A project, a question, a quote? Describe it and we will answer by email.",
    name: "Name",
    organization: "Organisation",
    email: "Email",
    topic: "Subject",
    topics: { quote: "Quote request", question: "Product question", security: "Security", other: "Other" },
    workstations: "Expected number of workstations",
    message: "Message",
    send: "Send",
    privacy: "Your details are only used to answer you.",
    sent: "Message sent. Thank you, we will get back to you shortly.",
    invalid: "Please check the fields: name, email and message are required.",
    failed: "Sending failed. Please try again later.",
  },
  legal: {
    pending: "Document in preparation: it will be published before sales open.",
    notice: "Legal notice",
    privacy: "Privacy",
    terms: "Terms of sale",
  },
  footer: {
    madeIn: "Designed in Europe",
    affiliation: "Oblivio is not affiliated with NIST or IEEE; their standards are cited as references.",
  },
};

export default en;
