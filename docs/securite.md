# disphas — Modèle de menaces et revue de sécurité

État au **2026-10-04**. Complète le [cadre](cadre_projet.md) (modèle de confiance du certificat), le
[protocole](protocole_ipc.md) (frontière UI ↔ helper) et la [conformité](conformite.md) (CRA, RGPD).

## 1. Ce qu'on protège

| Bien | Pourquoi |
|---|---|
| **Les disques non visés** | Un effacement est irréversible : ne jamais toucher un disque protégé ou autre que celui confirmé |
| **La vérité du certificat** | Un certificat CONFORME doit correspondre à un effacement réellement fait et prouvé |
| **La clé du poste** (`/var/lib/disphas/keys`) | Elle signe les certificats ; volée, elle permet de forger des certificats au nom du poste jusqu'à révocation |
| **La clé racine** (hors ligne, chez l'éditeur) | Elle signe licences et listes de révocation ; volée, elle permet tout |
| **La clé boutique** (serveur de l'éditeur ; décidé le 2026-10-11, non implémenté) | Elle signe les licences v2 ; volée, elle permet de forger licences et certificats jusqu'à révocation de l'émetteur ([licences en ligne](licences_en_ligne.md) §7) |
| **Le poste lui-même** | Le helper tourne en root : il ne doit pas devenir une porte d'entrée vers root |
| **Les données des certificats** | Données personnelles possibles (opérateur, n° de série) : root seul, lues par le helper (authentification administrateur) |

## 2. Qui pourrait attaquer

| Acteur | Capacités supposées |
|---|---|
| A1 — Programme malveillant **dans la session de l'opérateur** (même compte, sans root) | Lit et écrit les fichiers de l'utilisateur, peut lancer l'application, peut éventuellement piloter le processus de l'application (`ptrace`) selon la configuration du noyau |
| A2 — **Fichier piégé** soumis à disphas | Certificat PDF/COSE/QR à vérifier, logo, licence, liste de révocation, demande d'activation |
| A3 — **Disque défectueux ou hostile** | Réponses de firmware fausses ou malformées (IDENTIFY, journaux, SMART), comportement mensonger |
| A4 — **Autre compte local** sans droits | Lit ce qui est lisible par tous, crée des fichiers là où c'est permis |
| A5 — **Faussaire de certificats** | N'a pas accès au poste ; fabrique ou modifie des certificats et les présente à un tiers |
| A6 — **Chaîne d'approvisionnement** | Dépendance compromise, paquet modifié en transit |
| A7 — **Réseau et serveur de licences** (dès l'activation en ligne, décidée le 2026-10-11) | Intercepte ou usurpe le serveur de licences, renvoie des réponses piégées ; ou compromet le serveur et vole la clé boutique |

Hors périmètre V1 : un attaquant déjà **root** sur le poste (il peut tout, y compris lire la clé du poste —
limite assumée, voir [certificat](certificat.md) §4) ; l'accès physique prolongé à la machine.

## 3. Frontières de confiance

```
utilisateur / fichiers reçus ──► application (compte utilisateur) ──pipe JSON──► helper (root) ──ioctl──► disques
                                         │                                        │
                                         └─► ~/Documents/disphas, ~/.config        └─► /var/lib/disphas
```

- **Application → helper** : le helper ne fait **aucune** confiance à l'application (cadre §2) ; il
  revérifie tout (identité du disque relue par lui, contrôles de `Sanitize` — 10 depuis le 2026-10-20 —,
  licence, droits de l'offre, bornes).
- **Fichiers reçus → application** : toute entrée est bornée en taille et vérifiée avant affichage ; un
  certificat qui ne se vérifie pas n'est jamais affiché comme fait.
- **Disques → helper** : les données de firmware sont décodées de façon défensive (fuzzées).
- **Poste → tiers** : seul le certificat signé fait foi ; le PDF n'est qu'un rendu.
- **Application → bureau** (depuis le 2026-10-28, désactivé par défaut) : la notification de fin d'effacement
  passe par le service de notifications du bureau (D-Bus de session sous Linux), jamais par le réseau. Elle ne
  contient que des nombres et des statuts, aucun n° de série. Seule l'interface, sans droits root, l'envoie ; le
  helper n'y touche pas.
- **Serveur de licences → application** (décidé le 2026-10-11, non implémenté) : réponse bornée, une seule URL
  HTTPS, code réseau sans droits root ; la licence reçue n'est crue qu'après vérification de sa signature par le
  helper. Un faux serveur peut bloquer une activation, pas forger une licence
  ([licences en ligne](licences_en_ligne.md) §5, §7).

## 4. Limites assumées (V1)

- **`iat` déclaré par le poste** : l'horodatage n'est pas certifié par un tiers ([certificat](certificat.md) §4).
- **Clé du poste non scellée** : fichier 0600 root ; volée par un attaquant root ou par vol du disque.
- **Clé boutique volée** (dès les licences en ligne) : le faussaire peut dater ses licences et certificats avant la
  révocation de l'émetteur, de même que `iat` est déclaré par le poste ([licences en ligne](licences_en_ligne.md) §7).
- **A1 et le pipe** : un programme du même compte qui contrôle le processus de l'application peut envoyer
  des commandes au helper pendant la session. Le helper applique alors toutes ses protections (la confirmation
  par le n° de série, facultative depuis le protocole 3, n'a jamais arrêté ce cas : le n° de série est lisible
  par `ListDevices`) ; il ne peut pas effacer un disque protégé ni signer un faux
  résultat, mais il peut effacer un disque **non protégé** branché. Réduction : `kernel.yama.ptrace_scope ≥ 1`
  (valeur constatée sur Pop!_OS 24.04 ; à vérifier sur chaque distribution cible), session dédiée à l'opérateur.
- **Logo du client contrôlé par l'application seulement** ([offres](offres.md) §4.5, Q3) : le PDF est rendu par
  l'application, qui n'est pas une frontière de confiance. Quelqu'un qui la modifie peut imprimer son logo
  sans l'offre qui l'inclut. Assumé : le logo est cosmétique, il ne figure pas dans l'attestation signée et ne
  change rien à la preuve.
- **Limite de parallélisme** ([offres](offres.md) §4.8) : limite commerciale, pas une barrière de sécurité ;
  un administrateur qui modifie le helper ou les fichiers de `/var/lib/disphas` la contourne.

## 5. Revue de sécurité du 2026-10-04

Cinq relectures en parallèle (protocole et session, processus et fichiers, couche disque, application,
cryptographie), en lecture seule. Chaque constat retenu ci-dessous a été **revérifié dans le code** ; les
autres sont marqués *à confirmer*. Aucune faille mémoire, aucun contournement des 9 contrôles de `Sanitize`,
de la licence ou du refus des disques PROTÉGÉS depuis le protocole, aucune injection dans le PDF ni XSS.

### 5.1 Élevés (✅ corrigés le 2026-10-04)

- **S1** : le paquet crée `/var/lib/disphas` et ses sous-dossiers (root 0700 ; depuis le 2026-10-06 `reports` aussi, lu par le helper) et s'arrête s'il y trouve un lien symbolique ; le helper fixe `umask 077` au démarrage et **refuse de démarrer** si le dossier d'état ou un sous-dossier n'est pas un vrai dossier appartenant au compte du helper et fermé en écriture aux autres ; les certificats sont créés en 0600, root seul. Vérifié : refus réel sur un dossier 0777, démarrage normal sinon ; droits contrôlés après installation par le workflow *Package*.
- **S2** : après l'ouverture exclusive, le numéro de périphérique doit être celui de l'inventaire et l'identité relue par IDENTIFY sur le descripteur ouvert doit être exactement celle confirmée ; même contrôle avant un déverrouillage.


| # | Constat | Vérifié | Correction |
|---|---|---|---|
| S1 ✅ | `/var/lib/disphas` est créé par le helper avec les droits issus du `umask` hérité ; ni le paquet ni le helper ne fixent ses droits ni ne vérifient son propriétaire. Si un utilisateur en obtient l'écriture : clé du poste substituée (le contrôle accepte le propriétaire du dossier parent), `reports` en lien symbolique (chmod/chown root ailleurs), tâche plantée reprise sans confirmation | Oui (transmission du `umask` par pkexec *à confirmer*) | Le paquet crée l'arborescence root ; le helper fixe `umask 077`, refuse tout dossier d'état qui n'est pas à root, est un lien, ou est ouvert en écriture à d'autres ; clé du poste exigée à root |
| S2 ✅ | Le disque est ouvert **par son nom** après résolution de l'identité, sans revérifier ensuite que c'est le même disque (débranchement / branchement entre-temps) | Oui | Après ouverture exclusive : comparer le numéro de périphérique et relire l'identité (IDENTIFY) sur le descripteur ouvert |

### 5.2 Moyens

Corrigés le 2026-10-04 :

- **S3** : à l'ouverture d'un PDF, aucun flux décodé d'office (flux d'objets, tables de références) ne peut dépasser 4 Mio décompressés (`LoadOptions::max_decompressed_size` de `lopdf`) ; un flux plus gros est ignoré. Les PDF produits par disphas n'en contiennent pas (Typst n'émet pas de flux d'objets). Le PDF n'est plus analysé qu'une fois pour choisir entre certificat seul et certificat de lot. Testé : une bombe de quelques Kio gonflant à 64 Mio n'est pas décodée, et le même test échoue sans la limite.
- **S4** : la file de sortie du helper est bornée (256 messages) ; une réponse attend une place (le helper cesse alors de lire les requêtes), un événement de tâche est abandonné plutôt que de bloquer la tâche ; le détail d'erreur renvoyé est tronqué à 256 octets. Testé : un client qui ne lit pas bloque le helper après moins de la moitié de 1024 requêtes (le test échoue avec une file non bornée) ; une commande inconnue de 512 Kio n'est pas renvoyée en entier.
- **S5** : le bit `FNA` 1 est porté par le profil et le verdict ; un Format NVM sur un tel contrôleur exige `NS_ALL` comme un Sanitize. Pendant un Sanitize ou un tel Format (y compris à la reprise après un arrêt), les autres namespaces du contrôleur sont ouverts en `O_EXCL` et l'opération est refusée si l'un est utilisé. Testé : portée du verdict, liste des namespaces voisins, portée transmise à l'ouverture (Sanitize, reprise, écriture hôte). L'ouverture exclusive des voisins sur un vrai disque reste à vérifier au banc. Limite : un namespace attaché mais non exposé par le noyau ne peut pas être tenu.
- **S6** : une seule recherche de disque par identité (`Inspect`, `Sanitize`, `Unlock`) refuse quand plusieurs disques répondent avec la même identité ou quand le disque n'a pas de n° de série ; le contrôle 4 refuse un n° de série vide (une confirmation vide aurait suffi). Un enregistrement de récupération Security Erase n'est appliqué qu'au seul disque de même modèle, n° de série et WWN, identité relue ensuite sur le disque ouvert. Testé, y compris que la confirmation vide passait avant la correction ; les tests du vrai backend tournent désormais en CI (`--features real-devices`, sans ouvrir de disque).
- **S7** : contrôle de cohérence de l'attestation ([certificat](certificat.md) §2) au décodage, donc à toute vérification (application, `disphas-verify`, lots), et dans le helper avant signature. `z` n'est plus posé que si la réécriture et la passe de zéros sont vérifiées et validées, jamais sur un REFUS. Testé : 14 contradictions signées rejetées, les 5 formes produites par disphas acceptées ; un disque qui ignore les zéros donne un REFUS sans `z`, et avec l'ancien code la tâche aurait été arrêtée par le contrôle avant signature.
- **S8** : identifiants de licence `DSL-AAAAMMJJ-` + 64 bits aléatoires ; un identifiant déjà au registre est refusé avant toute signature. `revoke` exige la dernière liste (`--list`) ou `--first` pour la toute première, refusé dès qu'une liste est livrée, comme une liste plus ancienne que celle livrée. Testé (format des identifiants, règles de base de la liste).

| # | Constat | Vérifié | Correction |
|---|---|---|---|
| S3 ✅ | PDF piégé : la décompression des flux d'objets n'est pas bornée à l'ouverture (`lopdf` sans limite par défaut) → saturation mémoire de l'application / `disphas-verify` | Oui (source de lopdf) | Limite de décompression à l'ouverture ; une seule analyse |
| S4 ✅ | File de réponses du helper sans limite, commande inconnue renvoyée en entier → saturation mémoire du helper root par un client qui ne lit pas | Oui | File bornée ; écho tronqué |
| S5 ✅ | NVMe Format peut effacer **tous** les espaces de noms (bit FNA lu mais inutilisé) ; les espaces de noms voisins ne sont pas tenus pendant une commande à l'échelle du contrôleur | Oui | Portée « tous les namespaces » aussi pour Format quand FNA l'indique ; ouverture exclusive des voisins pendant l'opération |
| S6 ✅ | Identités en double (n° de série identique ou vide) : le premier disque correspondant est pris ; la reprise Security Erase ne compare que le n° de série | Oui | Refuser une identité ambiguë ou un n° de série vide ; comparer l'identité complète |
| S7 ✅ | Rien ne revérifie la **cohérence** d'un certificat (ex. réécriture hôte certifiée Purge) ; l'option zéros est déclarée « validée » même si la validation échoue | Oui | Contrôle de cohérence avant signature et à la vérification ; `z` seulement si validé |
| S8 ✅ | Outil éditeur : identifiants de licence sur 16 bits, unicité non vérifiée (la révocation porte sur l'identifiant) ; `revoke` sans `--list` oublie les révocations précédentes | Oui | Identifiants longs et uniques ; `--list` obligatoire dès qu'une liste existe |
| S9 ✅ | Utilisateurs cachés d'un disque : périphérique loop adossé à une partition, swap sur loop, accès direct par un autre processus | *À confirmer* (comportement noyau) — **confirmé sur le banc le 2026-10-07** ([banc de test](banc_de_test.md)) | Suivre les loop vers leur disque ; refuser si un processus tient le disque ouvert |
| S10 ✅ | NVMe sur réseau (NVMe-oF) traité comme un disque local | Plausible | N'accepter que le transport `pcie` |
| S11 ✅ | Partition EFI sur un autre disque, montée à la demande (autofs) : disque système non protégé | *À confirmer* | Traiter autofs sur les points système comme non traçable (refus) |

Corrigés le 2026-10-04 (suite) :

- **S9** : un loop adossé à un disque ou à une partition protège ce disque (P4), un montage via un tel loop est rattaché au disque ; un loop lié dont la source est illisible fait échouer l'inventaire. À l'ouverture pour effacement, refus si un autre processus tient le disque, une partition ou un namespace voisin ouvert (`/proc/<pid>/fd`, comparaison par numéro de périphérique). Limite : un programme qui ouvre le disque **après** ce contrôle sans `O_EXCL` n'est pas arrêté.
- **S10** : un NVMe n'est un disque local que si son contrôleur (ou chacun des contrôleurs en multipath) annonce le transport `pcie` ; sinon il n'est jamais ouvert pour effacement. Vérifié sur le poste de développement (`pcie`).
- **S11** : un point de montage système en automount est lu pour déclencher son montage ; s'il reste non monté, l'inventaire échoue (le disque système serait inconnu). Un automount hors points système (`binfmt_misc`) est ignoré — vérifié sur le poste de développement.

Testés sur arborescence simulée ; le comportement réel des loop et du déclenchement d'automount reste à confirmer au banc.

### 5.3 Faibles et informatifs (extraits)

Corrigés le 2026-10-04 :

- **L1 textes** : opérateur, organisation, justification (≤ 500) et libellé de lot refusent les caractères de contrôle et de mise en forme invisibles ou d'inversion de sens, au helper et au décodage de toute attestation.
- **L2 tâche qui panique** : la panique est rattrapée et termine la tâche en échec, sans attestation, fichiers gardés pour le prochain démarrage comme après un arrêt brutal ; la table des tâches, empoisonnée par une panique, est récupérée : le disque n'est plus « occupé » jusqu'à la fin de la session. Testé (le test échoue sans la correction).
- **L3 verdicts périmés** : le lancement d'une tâche invalide tous les verdicts du même disque, pas seulement celui utilisé. Testé.
- **L4 onglet Vérifier** : un journal joint qui ne correspond pas à l'attestation donne un en-tête rouge « Attestation authentique, journal non conforme », comme `disphas-verify` qui renvoie alors un échec (code 1).
- **L5 lots mêlant plusieurs licences** : prévu par la spec (plusieurs postes d'un client) ; le risque réel est un même numéro de lot chez deux clients. Un disque sous licence d'un autre client que le premier est refusé, à la production du PDF de lot comme à la vérification (application, `disphas-verify`). Testé.
- **L6 attente Sanitize** : une opération dont l'avancement ne bouge plus pendant 4 heures est abandonnée (tâche en échec, sans certificat CONFORME) au lieu d'être attendue indéfiniment. Le délai porte sur l'absence de progrès, pas sur la durée totale : un long effacement qui avance n'est jamais interrompu. Testé (horloge simulée).
- **L7 transfert ATA incomplet** : une commande ATA dont SG_IO signale des octets non transférés (`resid`) est en échec (un IDENTIFY tronqué était décodé avec une fin vide). À confirmer au banc qu'aucun contrôleur SATA courant ne signale de `resid` à tort.
- **L8 inventaire fail open** : trois chemins qui laissaient un disque paraître libre échouent désormais : montage d'un numéro de périphérique inconnu de sysfs, pile `slaves/` illisible ou trop profonde, montage ZFS (même hors racine). Testés ; inventaire réel du poste de développement inchangé.
- **L9 copies du mot de passe ATA** : supprimées dans le helper : décodage des requêtes sans arbre JSON intermédiaire (les arguments restent une tranche de la ligne, puis sont décodés depuis un tampon effacé), bloc ATA de 512 octets écrit directement dans un tampon effacé, enregistrement de récupération lu dans des chaînes effacées ; la ligne envoyée par l'application est effacée après écriture. Limites : la chaîne saisie dans l'interface (JavaScript) ne peut pas être effacée ; un mot de passe contenant des échappements JSON (`\\`, `\"`, `\u…`) passe par un tampon interne de `serde_json` non effacé.
- **L10 enveloppe COSE ré-encodable** : l'enveloppe (attestation, licence, liste de révocation) doit se réencoder à l'octet près ; une variante d'encodage, qui changeait l'empreinte du fichier sans toucher la signature, est rejetée. Testé (le test échoue sans la correction).
- **L11 `--root-key` officielle** : la clé officielle donnée explicitement reste officielle : la liste de révocation intégrée s'applique (elle était ignorée, un certificat d'une licence révoquée passait) et le rapport n'affiche plus l'avertissement « non officielle ». Testé.
- **L12 temps d'effacement ATA « 255 »** : la valeur 255 du format court signifie « plus de 508 minutes » ; elle était lue 510 minutes, d'où un délai de commande de 17 h qui pouvait couper un effacement plus long et laisser le disque verrouillé. Elle compte désormais pour 36 h d'estimation (délai 72 h). Testé.

Constats d'origine (tous traités ci-dessus, L1 à L12) : textes déclaratifs non bornés, tâche qui panique, verdicts
périmés, onglet *Vérifier* vert malgré un journal non conforme, lots multi-licences, attente Sanitize sans fin,
transfert ATA incomplet, inventaire *fail open*, copies du mot de passe ATA, enveloppe COSE ré-encodable,
`--root-key` officielle, temps d'effacement « 255 ».

### 5.4 Contrôles jugés corrects

Chaîne de signature (`verify_strict`, en-têtes COSE stricts, CBOR canonique re-encodé), exclusion de la racine
de développement des builds release, 9 contrôles de `Sanitize` dans l'ordre avec identité relue, licence
revérifiée, `O_EXCL` tenu toute la tâche, structures `ioctl` conformes aux en-têtes du noyau, décodeurs bornés,
séquence Security Erase, politique polkit (`auth_admin`, jamais conservée), absence d'injection Typst et de XSS,
CSP sans source distante, chemins de fichiers de l'application contrôlés, listes de révocation *fail closed*.

