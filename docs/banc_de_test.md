# Banc de test matériel

> Prérequis de toute validation sur vrai disque (CLAUDE.md §2). Jamais sur la machine de développement principale.

## Matériel

- Une machine dédiée (ou un emplacement **interne** libre), sans données à protéger.
- Disques **sacrifiables**, en connexion directe (SATA ou M.2 ; pas d'adaptateur USB, hors périmètre V1) :
  au minimum un SSD SATA avec Sanitize, un NVMe avec Sanitize, idéalement un HDD SATA et un SSD sans Sanitize.
- `smartmontools` installé (oracle de test, cadre §9).

## Étapes

1. **Inspection** (lecture seule) du disque de test, en comparant à `smartctl --json -x` :
   ```bash
   sudo -E env OBLIVIO_TEST_DEVICE=sdb OBLIVIO_TEST_SERIAL=<numéro de série> \
     cargo test -p oblivio-device --features hw-tests --test hw -- --nocapture
   ```
   Vérifier modèle, série, firmware, capacité, capacités Sanitize, état Security, attributs SMART.
2. **Écriture directe** (destructif, bloc final du disque) :
   ajouter `OBLIVIO_TEST_DESTRUCTIVE=I-ACCEPT-DATA-LOSS-ON-THE-TEST-DRIVE` et `-- --ignored`.
3. **Parcours complet** avec un helper `--features real-devices` lancé en root, puis l'application :
   inspection, verdict, effacement, attestation, PDF, `oblivio-verify`.
4. Pour chaque méthode firmware : vérifier la progression, la validation par canaris, et provoquer un échec
   (débrancher pendant un Sanitize sur un disque sacrifié) pour observer B4/B5 au redémarrage.

## Points à confirmer en priorité

- Offsets et bits décodés (FNA, SANICAP, SANITIZE STATUS EXT bits 13/14) — `moteur_decision.md` §4.
- Comportement de l'état du journal Sanitize juste après le démarrage d'un Crypto Erase rapide.
- Temps réels d'effacement par méthode, pour les estimations de l'interface.
