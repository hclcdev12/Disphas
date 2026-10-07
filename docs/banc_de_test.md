# Banc de test matériel

> Prérequis de toute validation sur vrai disque (CLAUDE.md §2). Jamais sur la machine de développement principale.

## Matériel

- Une machine dédiée (ou un emplacement **interne** libre), sans données à protéger.
- Disques **sacrifiables**, en connexion directe (SATA ou M.2 ; pas d'adaptateur USB, hors périmètre V1) :
  au minimum un SSD SATA avec Sanitize, un NVMe avec Sanitize, idéalement un HDD SATA et un SSD sans Sanitize.
- `smartmontools` installé (oracle de test, cadre §9).

**Banc actuel (2026-10-06)** : Dell Precision T3500 (SATA Intel ICH10 en AHCI, PCIe 2.0, pas de M.2), démarré en
**live PXE** (squashfs sur NFS, couche d'écriture en RAM) ; SSD SK hynix SC210 128 Go (sda), HDD Hitachi
HTS723232A7A364 320 Go (sdb), HDD Toshiba MK1655GSX 160 Go (sdc). NVMe à venir sur carte PCIe → M.2 passive
(un pont USB RTL9210B ne laisse passer qu'Identify et Get Log Page : inutilisable, cf. smartmontools).
- **Pilotage à distance (2026-10-06)** : après chaque démarrage, le propriétaire lance `sudo sh tools/bench/setup.sh`
  sur le banc (root sans mot de passe pour `goupil`, helper Oblivio sans demande polkit, `xdotool`). Claude voit l'écran
  par SSH (`xwd` sur l'affichage `:0`), pilote l'interface (`xdotool`) et agit en root selon l'exception du banc
  (CLAUDE.md §2). La licence reste à signer par le propriétaire après chaque redémarrage.
- **Inspection en lecture seule (2026-10-06)** : identité, capacité, WWN, état Security, SMART et absence de
  Sanitize décodés par Oblivio identiques à `smartctl -x` et aux mots bruts de `smartctl --identify` sur les trois
  disques. Le Toshiba (sdc) ne renseigne pas le mot 217 : il était classé HDD via sysfs, il est désormais traité
  comme SSD (Security Erase certifié Clear). Temps estimés (mots 89/90) : sda 2/2 min, sdb 76/78 min, sdc 70 min/—.
- `/var/lib/oblivio` est en RAM : licence, clé du poste et enregistrements de récupération sont perdus à chaque
  redémarrage (option (a), décidée le 2026-10-06). **Pas de test de Security Erase avec coupure** sur ce banc tant
  que ce dossier n'est pas persistant.

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
- ATA Security Erase : disposition du bloc de 512 octets (contrôle en octets 0–1, mot de passe en 2–33,
  comme hdparm), désactivation de la sécurité après l'effacement, et **coupure volontaire pendant
  l'effacement** puis redémarrage du helper : le disque doit être déverrouillé et libéré au démarrage.
- `Unlock` sur un disque verrouillé avec un mot de passe connu, puis effacement.
- Reprise d'un Sanitize après une **coupure de courant** (NVMe et ATA) : le disque doit reprendre seul,
  l'inspection (IDENTIFY) doit répondre pendant le Sanitize, et la tâche doit finir CONFORME au redémarrage.
- Disque *Frozen* (B1) : vérifier qu'une mise en veille / réveil du poste le dégèle, pour le guide.
- ~~Paquet `.deb` : installation → `/var/lib/oblivio` root seul (0700), certificats d'une version antérieure passés en
  0600 ; historique et PDF de lot lus par le helper, sans groupe~~ → **validé le 2026-10-07** (paquet de `472cec3`,
  mise à jour depuis une version à groupe `oblivio`, groupe ensuite supprimé) : dossiers 0700 et 5 certificats passés
  de 0640 `root:oblivio` à 0600 root ; historique, réimpression, PDF de lot depuis l'historique ; effacement de sda
  (Security Erase, CONFORME) dans un lot puis clôture, PDF de lot vérifié par `oblivio-verify`. Constat : le groupe
  `oblivio` d'une ancienne version n'était supprimé ni par la mise à jour ni par `apt purge` → supprimé par le
  paquet depuis le 2026-10-07 (vérifié par la CI *Package* et sur le banc, paquet `0.1.0+build.11`).
  `apt remove` / `apt purge` : contrôlés par la CI (*Package*), pas sur le banc.
- Effacement parallèle sur plusieurs vrais disques (SATA et NVMe en même temps), puis certificat de lot.
- B5 (Sanitize en échec) : ajouter et valider la commande « Exit Failure Mode » (NVMe Sanitize action 1,
  ATA SANITIZE STATUS EXT avec Clear Sanitize Operation Failed), avec un bouton dans l'application ;
  d'ici là, la consigne oriente vers la destruction ou l'outil du constructeur (décidé le 2026-10-04).
- NVMe à **plusieurs namespaces** (S5) : un Sanitize, ou un Format sur un contrôleur annonçant `FNA` bit 1,
  doit tenir les autres namespaces en `O_EXCL` (un `mount` d'un voisin pendant l'opération doit échouer) et
  être refusé si un voisin est utilisé avant le départ ; le bit `FNA` 1 décodé doit correspondre à la fiche du disque.
- Loop (S9) : `losetup` d'une partition d'un disque de test → le disque doit passer PROTÉGÉ (P4) ; un `dd` ou une VM
  qui garde le disque ouvert → refus à l'ouverture (« open in another process »).
- Automount (S11) : sur un système où `/boot/efi` est en automount systemd, l'inventaire doit déclencher le
  montage et protéger le disque de l'ESP (P1).
- ATA (L7) : `resid` de SG_IO nul sur IDENTIFY, SMART et SANITIZE STATUS pour chaque contrôleur SATA du banc.
