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
| [Cahier des charges](docs/cahier_des_charges_logiciel_d_effacement_s_curis.md) | Spécifications techniques et fonctionnelles v2.0 |
| [Specs UI & frontend](docs/oblivio_sp_cifications_ui_et_architecture_frontend.md) | Stack frontend, composants, workflow écrans |
| [Charte graphique](docs/oblivio_charte_graphique.md) | Palette, typographie, formes |
| [Identité de marque](docs/Oblivio%20—%20Identité%20&%20Narration%20de%20Marque.md) | Manifeste, piliers, baselines |

En cas de contradiction, le **cadre du projet** prime.

## Statut

Conception V1 terminée. Prochaine étape : ossature du workspace Rust (voir `CLAUDE.md`).
