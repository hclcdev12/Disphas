# Oblivio — Reprise du projet (passation entre agents)

État au **2026-10-07**. À lire **en premier** par tout nouvel agent, juste après `CLAUDE.md`. Ce document
dit où en est le projet, comment travailler avec le propriétaire et comment piloter le banc de test. Les
specs restent dans les documents listés par `CLAUDE.md` §1 ; ce fichier ne les remplace pas.

## 1. Où en est le projet

Le produit V1 est **complet et testé sur du vrai matériel** pour SATA (SSD et HDD) :

- Helper root, moteur de décision, protections P1–P5, 9 contrôles de `Sanitize`, signature Ed25519, PDF/A,
  `oblivio-verify`, licences et révocation, lots et effacement parallèle, paquets `.deb`/`.rpm` (6 distributions
  testées en CI), application Tauri (Disques, Effacements, Historique, Vérifier, réglages).
- **Revue de sécurité terminée** (S1–S11, L1–L12) : voir [sécurité](securite.md) §5.
- **Banc de test (2026-10-06)** : effacements réels réussis et certificats vérifiés hors ligne :

| Disque | Méthode | Résultat |
|---|---|---|
| sdc Toshiba MK1655GSX | réécriture hôte (+ passe de zéros) | CONFORME, Clear, R7 |
| sdb Hitachi HTS723232A7A364 | ATA Security Erase renforcé (68 min) | CONFORME, **Purge**, R6 |
| sda SK hynix SC210 | ATA Security Erase renforcé (53 s) | CONFORME, Clear, R6 |
| sda + sdc en parallèle, en lot | Security Erase + réécriture | lot authentique, PDF de lot produit |
| sda, paquet de `472cec3`, lot sans groupe (2026-10-07) | ATA Security Erase renforcé (< 1 min) | CONFORME, Clear, R6 ; lot clos, PDF de lot authentique |

Décisions récentes prises sur le banc (toutes documentées dans les specs) :

- Racine *live* (PXE) suivie couche par couche : pas de disque système local si tout est en RAM/réseau
  ([moteur](moteur_decision.md) §1).
- Mot 217 ATA non déclaré → traité comme SSD, certifié avec le type `SATA` (« type non déclaré »)
  ([moteur](moteur_decision.md), [certificat](certificat.md) §2).
- Portée « tous les namespaces » signée (`san.nsa`) ([certificat](certificat.md) §2).
- Compte à rebours de l'estimation du disque pendant un Security Erase (aucun avancement firmware).
- **Certificats lus par le helper** (`ListReports`, `GetReport`), root seul, **plus de groupe `oblivio`**
  ([protocole](protocole_ipc.md) §10) — commit `472cec3`.

## 2. Points ouverts

**À faire ensuite (banc)**

1. ~~Tester le paquet de `472cec3` sur le banc~~ → **fait le 2026-10-07**, tout est conforme (détail dans
   [banc de test](banc_de_test.md)).
2. NVMe : brancher un SSD NVMe via une **carte PCIe → M.2 passive** (le banc n'a pas de M.2 ; un pont USB
   RTL9210B ne laisse passer qu'Identify et Get Log Page, cf. smartmontools `scsinvme.cpp`). Puis Sanitize,
   Format, `nsa`, reprise après coupure. Le propriétaire doit déclarer le disque sacrifiable (CLAUDE.md §2).
3. Reste de [banc de test](banc_de_test.md) : `Unlock` d'un disque verrouillé, disque *Frozen*, coupure de courant,
   Exit Failure Mode (B5), loop (S9), automount (S11), NVMe multi-namespaces (S5).
4. Persistance de `/var/lib/oblivio` sur le banc (option (b) : NFS `no_root_squash`) : prérequis de tout test de
   coupure pendant un Security Erase.

**Décisions à soumettre au propriétaire**

5. `dev.typ` est une liste fermée : un vérificateur plus ancien rejette un type nouveau (constaté avec `SATA`).
   Recommandation : le rendre tolérant comme les codes. Change le format public → décision du propriétaire.
6. Chaque paquet doit avoir un **numéro de version distinct** : avec deux builds en 0.1.0, `apt` ne réinstalle
   pas (constaté sur le banc). À traiter avec le processus de release.
7. ~~Après un effacement, la carte du disque revient à l'ancienne décision~~ → corrigé le 2026-10-07 : l'app
   réinspectait déjà, mais la nouvelle décision est la même ; la carte affiche désormais le dernier effacement de la
   session et son statut. Vérifié sur le banc (Security Erase de sda, CONFORME).
8. Le groupe `oblivio` d'une version antérieure à `472cec3` n'est supprimé ni par la mise à jour ni par `apt purge`.
   Sans effet (certificats root seul) ; le supprimer dans `postinst` ? (constaté le 2026-10-07).

**Défauts d'interface constatés le 2026-10-07 (banc)**

9. ~~Historique : sélecteur de lot blanc sur blanc~~ → corrigé le 2026-10-07 (WebKitGTK dessinait les `<select>`
   avec le thème clair du système ; l'app les dessine elle-même, `color-scheme: dark`). Vérifié sur le banc ; le menu
   déroulant natif lui-même n'apparaît pas dans les captures `xwd`, non vérifié.
10. ~~Historique trié par identifiant (début) mais daté de la fin~~ → corrigé le 2026-10-07 : tri par la date affichée.
    Vérifié sur le banc.

**Plus tard** : dépôt public du vérificateur (depuis un **commit propre** : l'historique contient le n° de série
d'un vrai SSD), signature des paquets et release, vérification niveau audit, USB en V2, ISO live en V2.

**Côté propriétaire** : adresse security@, registre RGPD, validation juridique, achat IEEE 2883 puis ISO/IEC 27040,
CGV/EULA, relecture de l'allemand, site.

## 3. Travailler avec le propriétaire

- Il **ne code pas** : il supervise. Réponses en **français**, claires, avec le pourquoi des choix ; tableaux
  courts appréciés. Code, identifiants et commentaires en anglais.
- **On n'invente rien** : toute commande, option, chemin ou comportement est vérifié (code, config, doc
  officielle, exécution sans risque) avant d'être donné. Ce qui n'est pas vérifiable est dit comme tel.
- Il valide **étape par étape**. Une tâche finie → résumé + proposition de la suite, puis attendre son accord.
  Quand il dit « n'enchaîne pas », on s'arrête. Quand il dit « push seulement », on pousse sans rien lancer d'autre.
- **Push** uniquement sur son accord explicite (CLAUDE.md §5), après fmt + clippy + tests ; vérifier ensuite la CI et
  le dire. Le workflow *Package* se lance sur sa demande.
- Tout écart avec les specs ou décision qui les modifie : **le signaler et demander avant**. Mettre à jour `docs/`
  dans le même commit.
- Ne jamais manipuler ses mots de passe, la clé racine ni sa phrase secrète (même s'il les propose).

## 4. Le banc de test

Machine : Dell Precision T3500 (SATA ICH10 en AHCI, PCIe 2.0), **live PXE amnésique** (squashfs sur NFS
192.168.1.49, écriture en RAM), utilisateur `goupil`, 192.168.1.6, sur le même réseau que le poste de
développement. Disques déclarés sacrifiables : voir CLAUDE.md §2 (seuls ceux-là).

**Après chaque démarrage du banc** (tout est perdu à l'arrêt) :

1. Le propriétaire lance `sudo sh setup.sh` sur le banc (`tools/bench/setup.sh`, copié par SSH) : `sudo` sans mot
   de passe pour `goupil`, helper sans demande polkit, `xdotool`.
2. Copier et installer le paquet : `sudo apt-get install -y --reinstall /tmp/Oblivio_0.1.0_amd64.deb` (avec le chemin,
   sinon `apt` cherche un paquet de ce nom).
3. Lancer l'app sur l'écran du banc depuis SSH :
   `DISPLAY=:0 XAUTHORITY=$HOME/.Xauthority XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus setsid nohup /usr/bin/oblivio-app > /tmp/oblivio-app.log 2>&1 < /dev/null &`
4. Licence : l'app crée une demande `OBA1:…` (badge « Sans licence »). Le **propriétaire** la signe sur la machine
   qui détient la clé racine :
   `./target/release/oblivio-issuer issue <chemin>/root.key --customer "Banc Oblivio" --offer Pro --days 30 'OBA1:…'`
   (le registre `issued-licenses.csv` est à côté de `root.key`). La licence `OBL1:…` se colle dans l'app.

**Voir et piloter** :

- Capture : `tools/bench/shot.sh <sortie.png> [échelle]` (xwd par SSH, aucune installation sur le banc), puis lire
  l'image. Les coordonnées `xdotool` sont celles de l'écran plein (multiplier par l'échelle).
- Clic et saisie : `ssh oblivio-bench 'export DISPLAY=:0 XAUTHORITY=$HOME/.Xauthority; xdotool mousemove X Y click 1'`,
  `xdotool type --delay 80 TEXTE`. Agrandir la fenêtre : `xdotool search --name "^Oblivio$"` puis `windowsize`.
- Suivre une tâche : relever par de **courtes** connexions répétées (`sudo -n ls /var/lib/oblivio/jobs`,
  `/sys/block/sdX/stat`), jamais par une seule longue session SSH. Un Security Erase ne fait bouger aucun compteur :
  le fil du helper reste en état `D` jusqu'à la fin.
- Certificats : `sudo -n cat /var/lib/oblivio/reports/<id>.cose` (et `.json`), vérifiés avec
  `./target/release/oblivio-verify` **recompilé à jour** (un ancien binaire rejette les types récents).
- Test matériel bas niveau : `cargo test --release -p oblivio-device --features hw-tests --test hw --no-run`, copier
  le binaire, puis `OBLIVIO_TEST_DEVICE=sdX OBLIVIO_TEST_SERIAL=<n° de série> sudo -n <binaire> --nocapture`
  (`OBLIVIO_TEST_DESTRUCTIVE=I-ACCEPT-DATA-LOSS-ON-THE-TEST-DRIVE` et `--ignored` pour l'écriture).

## 5. Mettre en place un nouveau poste de développement

Prérequis vérifiés (documentation Tauri v2, *Prerequisites*, pour Fedora) :

```bash
sudo dnf check-update
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

- Rust : `rust-toolchain.toml` fixe le canal stable avec rustfmt et clippy (rustup l'applique seul).
- Node.js **24** (`.node-version`), par exemple avec fnm ; puis `npm ci` dans `app/`.
- `python3` (outils `tools/`), `pdftotext` pour relire un PDF en texte (paquet poppler).
- Clés **à recréer sur le nouveau poste** (ne pas copier les anciennes ; révoquer celles du poste précédent) :
  - clé de déploiement GitHub (accès en écriture) avec l'alias SSH `github-oblivio` (le dépôt pointe sur
    `git@github-oblivio:hclcdev12/Oblivio.git`) ;
  - jeton `gh` *fine-grained* sur le dépôt : Actions lecture/écriture, Contents lecture ;
  - clé du banc et alias `oblivio-bench` (`HostName 192.168.1.6`, `User goupil`), installée avec `ssh-copy-id`.
- **Clé racine** (`root.key`) et `issued-licenses.csv` : jamais dans le dépôt ; ne les placer sur un poste que si les
  licences y sont signées, transférés par le propriétaire lui-même.
- Constaté sur Fedora 44 (2026-10-07) : `wget` est fourni par `wget2-wget` ; le poste n'a besoin d'aucun droit root
  hors installation des dépendances. Au premier SSH vers le banc, comparer l'empreinte affichée à celle lue sur
  l'écran du banc (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`). Le banc étant amnésique, la clé installée par
  `ssh-copy-id` est sans doute perdue à chaque redémarrage (non vérifié) : piste, l'intégrer à l'image PXE avec `setup.sh`.
- Les agents ne saisissent **aucun** mot de passe (règle de sécurité de Claude, indépendante de ce dépôt) : tout accès
  passe par clé SSH ou `sudo` sans mot de passe préparé par le propriétaire.
- Vérifier le poste : `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, puis dans
  `app/` `npm run lint` et `npm run build`. Démonstration sans matériel : `npm run demo` dans `app/` (port 1420 libre).
