# Oblivio

Logiciel professionnel de sanitisation de supports de stockage, en Rust + Tauri v2.
Décision unique conforme NIST SP 800-88 Rev. 2 / IEEE 2883-2022, politique « Say NO »,
certificat d'audit signé Ed25519, 100 % offline.

> « Le droit à l'oubli, gravé au niveau métal. »

## Documentation

| Document | Contenu |
|---|---|
| [Cadre du projet](docs/cadre_projet.md) | **Référence** : périmètre V1, architecture, modèle de confiance, règles invariantes |
| [Moteur de décision](docs/moteur_decision.md) | Brouillon : états, critères Say NO, choix de méthode, validation |
| [Certificat](docs/certificat.md) | Brouillon : format signé, statuts, chaîne de confiance, vérification |
| [Protocole UI ↔ helper](docs/protocole_ipc.md) | Frontière de sécurité : commandes, contrôles, événements, reprise |
| [Banc de test](docs/banc_de_test.md) | Tests sur vrais disques : matériel, étapes, points à confirmer |
| [Cahier des charges](docs/cahier_des_charges_logiciel_d_effacement_s_curis.md) | Spécifications techniques et fonctionnelles v2.0 |
| [Specs UI & frontend](docs/oblivio_sp_cifications_ui_et_architecture_frontend.md) | Stack frontend, composants, workflow écrans |
| [Charte graphique](docs/oblivio_charte_graphique.md) | Palette, typographie, formes |
| [Identité de marque](docs/Oblivio%20—%20Identité%20&%20Narration%20de%20Marque.md) | Manifeste, piliers, baselines |

En cas de contradiction, le **cadre du projet** prime. La stack technique et ses versions sont décrites
dans le [cadre du projet, §2](docs/cadre_projet.md#stack-technique-état-au-2026-10-04).

## Statut (2026-10-04)

V1 fonctionnelle de bout en bout, **validée en simulation uniquement** : les commandes envoyées aux vrais
disques n'ont pas encore été confrontées au matériel (voir [banc de test](docs/banc_de_test.md)).

- Moteur de décision (états, Say NO, devenir, réserves, forçage) et évaluation après opération.
- Helper root : protocole, contrôles de `Sanitize`, exécution (NVMe Sanitize/Format, ATA Sanitize,
  ATA Security Erase, réécriture hôte), canaris et validation, signature, reprise après plantage.
- Licences hors ligne, révocation signée ; certificat CBOR/COSE, QR, PDF/A-3 en FR/EN/DE.
- Application : disques, effacement suivi, déverrouillage ATA, historique, vérification de certificats.
- `oblivio-verify` (public) : vérification hors ligne d'un PDF, d'un `.cose` ou d'un QR.
- Qualité : tests unitaires et de bout en bout, fuzzing (`fuzz/`), CI GitHub Actions.

## Démarrer

Prérequis : Rust stable (`rustup`), Node.js 24 (`fnm`), et pour Tauri sous Ubuntu/Pop!_OS :
`libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`.

| Commande | Effet |
|---|---|
| `cd app && npm ci && npm run demo` | Application sur des disques **fictifs** (fichiers image) : aucun vrai disque touché |
| `cd app && npm run dev` | Aperçu de l'interface dans un navigateur, sans helper |
| `cargo test --workspace` | Tous les tests (jamais sur un vrai disque) |
| `cargo run -p oblivio-verify -- certificat.pdf` | Vérifie un certificat signé par la clé **officielle**. Ceux de la démo (clé de développement) y sont refusés, c'est voulu : vérifiez-les dans l'onglet *Vérifier* de l'application de développement |
| `cd app && npm run tauri build` | Paquet `.deb` (helper réel + règle polkit) |

⚠️ Les tests sur vrais disques sont derrière la feature `hw-tests` **et** `OBLIVIO_TEST_DEVICE`, sur un
disque de test dédié uniquement (voir `CLAUDE.md` §2).
