# CLAUDE.md — Oblivio

Logiciel professionnel d'effacement sécurisé de disques (Rust + Tauri v2), conforme NIST SP 800-88 Rev. 2 / IEEE 2883-2022.
Décision unique, politique « Say NO », certificat signé Ed25519 vérifiable hors ligne, 100 % offline.

Le propriétaire du projet supervise des agents et n'écrit pas le code à la main : explique tes choix
clairement, signale tout écart avec les specs, et demande avant toute décision qui contredit `docs/`.

## 1. Documentation — à lire avant de coder

**Commencer par `docs/reprise.md`** : état d'avancement, points ouverts, façon de travailler avec le propriétaire,
pilotage du banc de test. Il ne remplace pas les specs ci-dessous.

Ordre de lecture et **ordre de priorité** en cas de contradiction :

1. `docs/cadre_projet.md` — **référence**. Périmètre V1, architecture, règles invariantes.
2. `docs/moteur_decision.md` — états, critères Say NO / devenir / réserves, choix de méthode, validation.
3. `docs/protocole_ipc.md` — frontière UI ↔ helper root.
4. `docs/certificat.md` — attestation CBOR/COSE, statuts, chaîne de signature.
5. `docs/cahier_des_charges_logiciel_d_effacement_s_curis.md`, `docs/oblivio_sp_cifications_ui_et_architecture_frontend.md`,
   `docs/oblivio_charte_graphique.md`, `docs/Oblivio — Identité & Narration de Marque.md` — documents d'origine,
   **supplantés** par 1 à 4 là où ils divergent.

Les docs sont en français. Le code, les identifiants et les commentaires sont en **anglais**.

## 2. Sécurité pendant le développement — RÈGLE ABSOLUE

Ce projet écrit du code capable de détruire des données. **Tu travailles sur la machine personnelle du propriétaire.**

- **Ne jamais** exécuter de commande ou de test qui écrit, efface, formate, sanitize ou envoie un ioctl
  d'administration vers un vrai périphérique (`/dev/sd*`, `/dev/nvme*`, `/dev/hd*`, `/dev/mmcblk*`…).
- **Ne jamais** lancer le helper ou un test avec `sudo`/`pkexec` sans accord explicite du propriétaire, pour ce lancement précis.
- Tests d'I/O uniquement sur des **fichiers image** ou des **loop devices** créés pour l'occasion.
- Les tests matériels réels sont derrière la feature `hw-tests` **et** la variable `OBLIVIO_TEST_DEVICE`,
  qui doit désigner explicitement un disque de test ; ils ne tournent jamais par défaut ni en CI.
- `oblivio-device` doit refuser par construction un disque PROTÉGÉ, même en test.
- **Exception : le banc de test** (`oblivio-bench`, 192.168.1.6, système amnésique démarré en PXE ; autorisé par le
  propriétaire le 2026-10-06). Lancements root, tests destructifs et effacements y sont autorisés **sans accord au cas
  par cas**, **uniquement** sur les disques de test déclarés sacrifiables par le propriétaire, identifiés par leur
  n° de série : sda `EI53N01701160366G`, sdb `E38345631J6UDN`, sdc `X9AHTNIET`. Le n° de série est vérifié avant
  toute écriture. Tout autre disque, et toute autre machine (dont le poste de développement), restent soumis aux
  règles ci-dessus. Chaque opération est rapportée au propriétaire. Pas de test de coupure pendant un Security Erase
  tant que `/var/lib/oblivio` n'est pas persistant. Les mots de passe et la clé racine restent au propriétaire :
  l'accès root passe par la règle `NOPASSWD` de `tools/bench/setup.sh`, que lui seul installe.

## 3. Règles invariantes du produit (ne jamais enfreindre)

1. Une écriture depuis l'hôte n'est **jamais** certifiée *Purge*. Seules les commandes firmware le sont.
2. Le **moteur de décision, les protections et la signature tournent uniquement dans le helper root**. L'UI ne décide rien.
3. Le helper identifie un disque par son **identité** (S/N + WWN/EUI-64/NGUID), jamais par un chemin `/dev/…` reçu de l'UI.
4. **Aucun outil externe** (`hdparm`, `nvme-cli`, `smartctl`, `dd`…) dans la chaîne certifiée : ioctl directs uniquement.
   `smartctl` n'est toléré qu'en oracle de test ou en avis secondaire optionnel (cadre §9).
5. Validation et signature ne sont **jamais** exposées comme commandes séparées : elles font partie de `Sanitize`.
6. L'attestation signée ne contient **aucun texte traduit**, uniquement des codes. Termes *Clear*/*Purge*/*Destruct* jamais traduits.
7. Tous les seuils du moteur sont des paramètres de `RuleSet` versionné (`rules_version`), jamais des constantes en dur.
8. Aucun appel réseau, aucune télémétrie, nulle part.
9. Secrets (clé privée, mot de passe ATA) : `zeroize`, jamais journalisés.

## 4. Organisation du code

```
oblivio/
├── CLAUDE.md
├── README.md
├── docs/
├── assets/                 logo, polices embarquées
├── public/                 → futur dépôt PUBLIC (Apache-2.0)
│   ├── oblivio-attestation/
│   └── oblivio-verify/
├── crates/                 → PRIVÉ
│   ├── oblivio-protocol/
│   ├── oblivio-core/
│   ├── oblivio-device/
│   ├── oblivio-license/
│   ├── oblivio-helper/
│   └── oblivio-report/
└── app/                    oblivio-app : Tauri v2 + React/Vite/Tailwind
```

**Dépendances autorisées :**
- `public/*` ne dépend de **rien** dans `crates/` ni `app/`. Elles seront extraites telles quelles en dépôt public.
- `oblivio-core` : aucun accès disque, aucune I/O. 100 % testable en unitaire.
- `oblivio-device` : seule crate qui parle aux périphériques.
- `oblivio-helper` : seul binaire root. Assemble protocol + core + device + license + attestation.
- `oblivio-report` et `app/` : jamais d'accès disque bas niveau, jamais de clé privée.

## 5. Conventions

- Rust stable récent, édition 2024, workspace Cargo unique. `cargo fmt` et `cargo clippy -- -D warnings` propres.
- `unsafe` limité à `oblivio-device` (ioctl, buffers alignés), chaque bloc commenté par `// SAFETY:`.
- Erreurs : `thiserror` dans les bibliothèques, pas de `unwrap()` hors tests.
- Désérialisation stricte (`#[serde(deny_unknown_fields)]`) pour tout ce qui traverse le protocole.
- **On n'invente rien** (règle universelle du propriétaire) : toute commande, option, chemin, API ou comportement
  donné au propriétaire est vérifié avant (code, configuration, documentation officielle, exécution sans risque).
  Ce qui ne peut pas être vérifié est présenté comme tel.
- Commits : Conventional Commits (`feat:`, `fix:`, `docs:`…), petits et ciblés.
- Toute décision qui modifie une spec → mettre à jour le document concerné dans `docs/` dans le même commit.
- **Push GitHub** : autorisé vers `origin` (dépôt privé), uniquement après fmt + clippy + tests verts en local,
  et à la fin d'une tâche validée par le propriétaire. Jamais de `--force`, jamais de réécriture d'historique,
  jamais vers un autre dépôt, jamais de dépôt public, sans accord explicite. Après chaque push, vérifier le
  résultat de la CI et le signaler.

## 6. Premières tâches (faites — l'état actuel est dans `docs/reprise.md`)

1. **Nettoyage** — supprimer les doublons périmés à la racine (les versions de `docs/` font foi) :
   - `cahier_des_charges_logiciel_d_effacement_s_curis.md`
   - `oblivio_sp_cifications_ui_et_architecture_frontend.md`
   - `oblivio_charte_graphique.md`
   - `Oblivio — Identité & Narration de Marque.md`

   Déplacer `gemini-svg.svg` vers `assets/logo/oblivio-logo-draft.svg`.
2. **Git** — `git init`, `.gitignore` (Rust, Node, Tauri), premier commit avec les docs.
3. **Ossature** — workspace Cargo avec les crates du §4 (vides mais compilables), `app/` laissé pour plus tard.
4. **`oblivio-core`** — types du verdict (`docs/moteur_decision.md` §6), `RuleSet`, fonction `decide()`
   couvrant les étapes 1 à 4 du moteur, avec tests unitaires sur des profils fictifs (un test par code P/B/N/D/R).

Faire valider par le propriétaire à la fin de chaque tâche avant de passer à la suivante.
