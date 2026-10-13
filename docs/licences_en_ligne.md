# disphas — Licences : émission en ligne, activation et renouvellement

> **Validé par le propriétaire le 2026-10-11. En cours d'implémentation** (§10) : format, vérification et
> installation par le helper de la licence v2, certification de la clé boutique (`disphas-issuer certify`), service
> de signature (`disphas-signer`) faits ; activation en ligne (application) et site à faire.
> D'ici là, les licences s'émettent et s'activent comme le décrit le [certificat](certificat.md) §4.
> Référence des licences, avec le [cadre](cadre_projet.md) §3 qui le résume.

## 1. Besoin (exprimé par le propriétaire le 2026-10-11)

1. Le client achète en ligne (Stripe) et obtient sa licence **sans intervention de l'éditeur**.
2. **Un seul copier-coller** : une clé de licence prise dans l'espace client, collée dans les réglages de disphas.
3. **Renouvellement automatique** tant que l'abonnement est payé, sans refaire la manipulation.
4. **Contrôle du nombre de postes** par abonnement.
5. Les données des disques et les certificats ne quittent **jamais** le poste.

Hors ligne strict, 2, 3 et 4 ne peuvent pas être réunis : compter les postes exige que le poste se présente au
site, et renouveler exige que le poste apprenne le paiement. D'où une **connexion limitée à la licence** (§4).

## 2. Décisions (validées le 2026-10-11)

| # | Décision | Remplace |
|---|---|---|
| D1 | **Clé boutique** : la racine (toujours hors ligne) signe un *certificat d'émetteur* ; le serveur signe les licences avec la clé boutique | Licences signées directement par la racine (cadre §3) |
| D2 | **Connexion limitée à la licence** : activation et renouvellement, depuis l'application (compte utilisateur), jamais depuis le helper | Règle 8 « aucun appel réseau, nulle part » |
| D3 | Licence **toujours liée au poste** (`wpk`), comme aujourd'hui | — (inchangé) |
| D4 | **Un identifiant de licence = un siège** : stable d'un renouvellement à l'autre | Un identifiant par licence émise |
| D5 | Licences émises par la clé boutique **toujours datées** (pas de perpétuelle) ; l'offre « à vie » est renouvelée gratuitement | — |
| D6 | **Secours hors ligne** conservé : demande `DSA1` collée dans l'espace client, licence recopiée à la main | — (le parcours actuel reste valable) |
| D7 | Licences v1 signées par la racine **toujours acceptées** (grands comptes, postes sans Internet, devis) | — |

**Règle 8 reformulée** (reportée dans `CLAUDE.md` §3) : *Aucun appel réseau ni télémétrie, à une exception près : l'application peut
contacter le serveur de licences de disphas pour activer ou renouveler la licence. Seuls transitent la clé de licence,
la clé publique du poste et l'identifiant de licence. Le helper n'ouvre jamais de connexion. Aucune donnée de disque
ni de certificat ne quitte le poste.*

## 3. Chaîne de confiance

```
Clé racine (hors ligne, éditeur)                     inchangée
   ├──signe──► Licence v1 DSL1 {…, wpk}               émission manuelle (disphas-issuer), toujours acceptée
   └──signe──► Certificat d'émetteur DSI1             1 par an, cérémonie hors ligne
                  {v: 1, iid, ipk, nbf, exp, off[], maxd}
                     └── clé boutique (serveur) ──signe──► Licence v2 DSL1 {v: 2, id, cli, off, nbf, exp, wpk, iss}
                                                              └── clé du poste ──signe──► Attestation (inchangée)
```

**Certificat d'émetteur** (CBOR déterministe, COSE_Sign1 par la racine ; `disphas_attestation::Issuer`) : `iid`
identifiant d'émetteur, **préfixe `ISS-` obligatoire**, 64 caractères au plus ; `ipk` clé publique Ed25519 de la
boutique ; `nbf`/`exp` validité, `exp > nbf` (1 an, avec chevauchement pour la rotation) ; `off` offres qu'il peut
émettre, non vide, triées, sans doublon ; `maxd` durée maximale d'une licence en secondes, non nulle (90 jours, le
plafond du §4).

**Licence v2** : les champs de la v1 avec `v: 2`, `exp` obligatoire, plus `iss` = le certificat d'émetteur signé
(octets du COSE_Sign1), **embarqué** pour que le `.lic` et le QR restent vérifiables seuls, hors ligne, avec la seule
clé racine. Une licence `v: 1` ne porte jamais `iss`. Aucun identifiant de licence (v1 ou v2) ne commence par `ISS-`.

**Vérification v2** (`disphas_attestation::verify_license`, utilisée par `verify`) : certificat d'émetteur signé par
la racine ; licence signée par `ipk` ; `off` ∈ `iss.off` ; `[nbf, exp]` de la licence inclus dans celui de
l'émetteur ; `exp − nbf ≤ maxd` ; ni la licence ni l'émetteur révoqués à la date `iat` de l'attestation
(`is_revoked`). Erreurs rendues avec les codes existants (`License`, `LicenseContent`, `LicenseRevoked`) : les
interfaces de vérification n'ont pas changé.

**Révocation** (simplifiée à l'implémentation, 2026-10-11) : la liste `DSR1` **reste en `v: 1`**. Ses entrées
`[[id, date], …]` désignent une licence **ou** un émetteur : le préfixe `ISS-`, réservé aux émetteurs et interdit aux
licences, lève toute ambiguïté. Mêmes règles (définitive, la date la plus ancienne l'emporte). Révoquer un siège =
révoquer son `id` ; révoquer un émetteur = révoquer son `iid`. `disphas-issuer revoke` et `disphas-verify` restent
compatibles.

**Taille du QR** (mesurée le 2026-10-11, certificat d'émetteur d'abord simulé, puis réel ;
QR niveau M ; taille d'un module = largeur imprimée / (modules + 8 de marge blanche)) :

| Certificat | Licence | Attestation signée | Texte du QR | Version QR | Module à 4,4 cm | Module à 5,5 cm |
|---|---|---|---|---|---|---|
| Typique (CONFORME, champs courts) | v1 | 749 o | 991 car. | 21 (101 modules) | 0,40 mm | 0,50 mm |
| Typique | v2 | 938 o | 1 232 car. | 23 (109 modules) | 0,38 mm | 0,47 mm |
| Pire cas (NON CONFORME, tous les textes au maximum, accents) | v1 | 1 826 o | 1 316 car. | 24 (113 modules) | 0,36 mm | 0,45 mm |
| Pire cas | v2 | 2 015 o | 1 564 car. | 27 (125 modules) | 0,33 mm | 0,41 mm |
| Pire cas, format v2 implémenté (`fixture::signed_largest`) | v2 | — | 1 540 car. | 26 (121 modules) | 0,34 mm | **0,43 mm** |

- La v2 ajoute **188 octets** (signature et clé de l'émetteur, incompressibles), soit **≈ 240 caractères**.
- Avec un identifiant de licence réel (`DSL-AAAAMMJJ-…`), le cas typique v1 est **déjà à 991 caractères** : le
  test actuel (`< 1000`, identifiant fictif court `LIC-0001`) sous-estime, et la cible « < 1 000 » est déjà
  dépassée aujourd'hui par le pire cas v1. La vraie contrainte est la **lisibilité du QR imprimé** (taille d'un module).
- **Décidé le 2026-10-11** : QR imprimé à **5,5 cm** (au lieu de 4,4 cm), pour que le pire cas v2 garde un module
  d'au moins 0,40 mm, celui d'un certificat typique à 4,4 cm. Garde-fou : test
  `largest_certificate_keeps_a_readable_qr_code` de `disphas-report` (certificat le plus long, `fixture::signed_largest`),
  qui échoue à 4,4 cm. **Reste à faire** : essai réel, impression du pire cas (`largest-fr.pdf` de l'exemple `sample`)
  et lecture au téléphone (le certificat de l'exemple porte déjà une licence v2).

Repli possible : le QR ne porte que `iid`, et les certificats d'émetteur sont livrés avec le vérificateur
(conséquence : un émetteur créé après la version du vérificateur n'est vérifiable qu'après sa mise à jour).

## 4. Parcours

### Activation (une fois par poste)

1. Espace client : l'abonnement affiche une **clé de licence** (secret aléatoire de 128 bits, en groupes lisibles)
   et le nombre de postes utilisés / autorisés.
2. disphas, Réglages → Licence : le client colle la clé, puis **Activer**.
3. L'application demande au helper une demande d'activation `DSA1` (commande existante), l'envoie au serveur avec la clé.
4. Le serveur vérifie la clé, l'abonnement actif et les postes disponibles, fait signer une licence v2 pour ce `wpk`
   et la renvoie. L'application la transmet au helper (`InstallLicense`, commande existante), qui la vérifie comme
   aujourd'hui.

### Renouvellement (automatique)

- Au lancement puis toutes les quelques heures, si la licence expire dans moins de **10 jours** et que le
  renouvellement en ligne est activé, l'application envoie une nouvelle `DSA1` et l'identifiant de licence.
- Abonnement payé → licence prolongée, même `id`, installée sans action du client. Sinon → rien ; la licence expire
  à sa date et disphas retombe en diagnostic seul (comportement actuel).
- Une licence renvoyée ne sert à rien sans la clé privée du poste, qui ne quitte jamais le poste : le renouvellement
  ne consomme pas de poste et n'expose rien.

### Durée des licences (paramètres du serveur, valeurs validées le 2026-10-11)

| Offre | Fin de licence |
|---|---|
| Mensuelle (Atelier) | Fin de la période payée + **7 jours** de marge |
| Annuelle / à vie (Pro) | Fin de la période payée + 7 jours, **plafonnée à 90 jours**, renouvelée tant que les droits courent |

Le plafond fixe le délai maximal entre la libération d'un poste dans l'espace client et l'arrêt effectif de sa licence.

### Changer de poste

Dans l'espace client, **Libérer** un poste : il n'est plus renouvelé et sa licence s'éteint à son échéance (au plus
environ 38 jours en mensuel, 90 jours sinon). En cas de vol ou de compromission, **Révoquer** : le siège entre dans la prochaine
liste de révocation (effective avec la mise à jour suivante).

### Secours hors ligne

Poste sans Internet : le client colle la demande `DSA1` dans l'espace client (Ajouter un poste) et recopie la licence
dans disphas, comme aujourd'hui. Les renouvellements se recopient de même depuis l'espace client (le serveur connaît
déjà le `wpk`, aucune nouvelle demande n'est nécessaire). Réglage **Renouvellement en ligne** : activé si
l'activation s'est faite en ligne, désactivable à tout moment.

## 5. Côté application

- Le code réseau est dans la partie **Rust de l'application** (compte utilisateur), pas dans la WebView : la CSP
  (`connect-src ipc: http://ipc.localhost`) reste inchangée.
- **Une seule URL**, compilée dans le binaire, HTTPS uniquement, aucune redirection suivie, réponses bornées en taille.
- **Proxy d'entreprise** (décidé le 2026-10-11) : pris en charge en V1 par les variables `HTTPS_PROXY` / `NO_PROXY`,
  un champ **Proxy** dans les réglages (l'application lancée depuis le menu ne reçoit souvent pas ces variables), et
  les **autorités de certification du système** (proxy qui inspecte le HTTPS avec un certificat interne). **Pas** en
  V1 : authentification NTLM/Kerberos, configuration automatique (PAC, WPAD) ; ces postes passent par le secours
  hors ligne (§4). Un proxy qui inspecte peut lire la clé de licence (il appartient à l'entreprise du client), mais
  ne peut pas forger de licence : la signature est toujours vérifiée.
- Le **helper ne change pas** : il ne voit que des textes `DSA1`/`DSL1`, comme aujourd'hui.
- Rappel local de l'échéance (« Votre licence expire dans N jours »), sans réseau.
- L'indicateur « HORS LIGNE » devient exact au mot près : affiché en permanence, sauf pendant une requête de licence.

## 6. Côté serveur

> §6.2 à §6.6 : **validé par le propriétaire le 2026-10-11** (débits proposés retenus). Chiffrement des clés au
> repos et outil d'export du registre : à trancher à l'implémentation.

### 6.1 Répartition

```
Internet ──HTTPS──► site Astro (licence.oblivio.eu, espace client, Stripe, base de données)
                        │  socket Unix locale, JSON (§6.3) — jamais exposée au réseau
                        ▼
                    disphas-signer (Rust, compte système dédié, aucun accès réseau)
                        └─ clés boutique, certificats DSI1, registre des licences
```

- Le **site** décide **qui** a droit à une licence : clé de licence, abonnement payé, postes disponibles, poste libéré.
- Le **service de signature** décide **ce qui peut être signé** : il ne connaît ni les clients ni Stripe, mais refuse
  tout ce qui sort des bornes du certificat d'émetteur et de ses propres règles (§6.4). Un site piraté ne peut donc
  obtenir que des licences bornées (offres prévues, 90 jours au plus), toutes inscrites au registre.

### 6.2 `disphas-signer` (binaire Rust, `tools/disphas-signer`, jamais livré aux clients)

Réutilise `disphas-license` et `disphas-attestation` : **une seule implémentation** du format. Commandes :

| Commande | Effet |
|---|---|
| `disphas-signer keygen` | Crée une nouvelle clé boutique (aléa de l'OS) et affiche sa clé publique (64 chiffres hexadécimaux), à porter sur la machine hors ligne pour `disphas-issuer certify` |
| `disphas-signer install 'DSI1:…'` | Vérifie le certificat avec la **racine officielle** (`decode_issuer`), exige que sa clé soit une clé boutique détenue ici, et l'enregistre |
| `disphas-signer status` | Certificats installés, dates, offres ; alerte si le certificat courant expire dans moins de 120 jours |
| `disphas-signer serve` | Sert la socket (§6.3) ; lancé par systemd |

- **État** dans `/var/lib/disphas-signer` (0700, compte `disphas-signer`) : `keys/` (graines 0600), `issuers/`
  (certificats `DSI1`), `register.csv` (§6.5). Clés **exclues des sauvegardes** du serveur : en cas de perte, on
  génère une clé et on la fait certifier, sans rien perdre d'autre.
- **Rotation** : une clé par certificat. Le certificat suivant (nouvelle clé) est installé **au moins 90 jours avant**
  l'expiration du courant ; le service signe avec le certificat valide qui expire le plus tard. Aucune licence n'est
  donc raccourcie par une fin de certificat.
- **Socket** `/run/disphas-signer/signer.sock`, mode 0660, groupe du site uniquement. Unité systemd durcie : pas de
  réseau (`PrivateNetwork=yes`), système de fichiers en lecture seule sauf l'état, pas de nouveaux privilèges.
- *Non tranché* : chiffrer les graines au repos (`systemd-creds`). Sans TPM sur le VPS, cela protège surtout les
  copies du disque ; à vérifier sur l'offre Infomaniak retenue.

### 6.3 Protocole site ↔ service (JSON, une requête par ligne)

Désérialisation stricte (champs inconnus refusés), réponses bornées, comme le protocole UI ↔ helper.

| Requête | Champs | Réponse |
|---|---|---|
| `Activate` | `request` (`DSA1:…`), `customer`, `offer`, `expires` | `id` (nouveau, `DSL-AAAAMMJJ-` + 64 bits), `workstation_key` (hex), `license` (`DSL1:…`) |
| `Renew` | `id`, `expires` | `license` (`DSL1:…`) — même `id`, même poste, même client, même offre, lus **dans le registre** |
| `Status` | — | Certificats installés, licences signées aujourd'hui / ce mois |

- `Activate` vérifie la preuve de possession de la demande (`read_activation_request`) : le site n'a jamais à
  décoder `DSA1` lui-même.
- `Renew` ne permet de changer **que la date de fin** : un site piraté ne peut pas déplacer une licence vers un
  autre poste ni changer d'offre.
- `not_before` est fixé par le service (heure courante − 60 s), jamais par le site.
- Erreurs : `BadRequest`, `UnknownLicense`, `OutOfBounds` (offre, durée, date), `RateLimited`, `NoIssuer`.

### 6.4 Règles appliquées par le service

1. Offre ∈ offres du certificat ; `expires − not_before ≤ maxd` ; `expires ≤` fin du certificat.
2. `customer` : 1 à 128 caractères, sans caractère de contrôle.
3. **Débit borné** (proposé) : 30 signatures par minute, 2 000 par jour ; au-delà `RateLimited` et alerte. Ordre de
   grandeur : 1 000 postes renouvelés une fois par mois ≈ 35 signatures par jour.
4. Chaque licence signée est inscrite au registre **avant** d'être renvoyée (écriture synchronisée).

### 6.5 Registre

- `register.csv` en ajout seul : `id, customer, offer, not_before, expires, workstation_key, issuer, signed_at`
  (mêmes colonnes que `issued-licenses.csv` de l'éditeur, plus l'émetteur). Un renouvellement ajoute une ligne.
- **Export quotidien** hors du serveur, chiffré pour l'éditeur (*outil à choisir*). Il sert à : rapprocher registre
  et base du site (une licence au registre sans siège payé = alerte), et réémettre les licences légitimes après
  incident (§7).

### 6.6 API publique de licences (`https://licence.oblivio.eu/v1/…`, servie par le site)

Contrat entre l'application et le site ; l'application ne connaît que ces deux appels.

| Appel | Corps | Réponses |
|---|---|---|
| `POST /v1/activate` | `{"key": "<clé de licence>", "request": "DSA1:…"}` | `200 {"license": "DSL1:…"}` ; `403 {"error": "invalid_key" \| "inactive" \| "no_seat"}` ; `400` |
| `POST /v1/renew` | `{"id": "DSL-…", "request": "DSA1:…"}` | `200 {"license": "DSL1:…"}` ; `200 {"renewed": false, "reason": "inactive" \| "released" \| "revoked"}` ; `400` |

- `renew` exige une demande `DSA1` **du même poste** (clé comparée au registre) : seul le poste concerné peut
  interroger sa licence ; la licence renvoyée reste inutilisable sans sa clé privée.
- Corps de 8 Kio au plus ; débit limité par adresse IP ; aucun cookie, aucun autre en-tête que `User-Agent:
  disphas/<version>`.
- Le **secours hors ligne** (§4) passe par l'espace client, qui appelle les mêmes `Activate` / `Renew` du service.

## 7. Sécurité

| Risque | Parade |
|---|---|
| Interception, faux serveur | HTTPS ; surtout, toute licence est vérifiée par signature : un faux serveur peut bloquer, pas forger |
| Réponse piégée | Code réseau sans droits root ; le helper reçoit un texte vérifié par le décodeur strict déjà fuzzé |
| Vol d'une clé de licence client | Consomme au plus les postes autorisés ; visible et libérable dans l'espace client |
| **Vol de la clé boutique** | Permet de forger des licences, donc des certificats, jusqu'à révocation de l'émetteur. Parades : clé isolée, validité 1 an, offres et durées bornées, registre hors serveur, procédure : révoquer l'émetteur, nouvelle clé, réémettre le registre (les postes en ligne la récupèrent seuls au renouvellement) |
| Antidatage après vol de la clé boutique | **Limite assumée**, de même nature que `iat` déclaré par le poste ([certificat](certificat.md) §4) : un faussaire peut dater ses faux avant la révocation |
| Serveur indisponible | Marge de 7 jours, puis secours hors ligne |
| Vie privée | Le serveur voit l'IP et les dates d'activation/renouvellement d'un poste : politique de confidentialité, journaux courts |

La clé racine ne touche jamais le réseau.

## 8. Points tranchés et points ouverts

- ~~Décisions D1 à D7, règle 8 reformulée~~ → **validées le 2026-10-11**.
- ~~Durées~~ → **validées le 2026-10-11** : marge 7 jours, plafond 90 jours, renouvellement 10 jours avant
  l'échéance, émetteur valable 1 an.
- ~~Proxy d'entreprise~~ → **décidé le 2026-10-11** : variables, champ des réglages et autorités du système en V1 ;
  NTLM/Kerberos et PAC/WPAD non (§5).
- ~~Nom de domaine~~ → **décidé le 2026-10-11** : domaine `oblivio.eu` ; serveur de licences sur le sous-domaine
  dédié **`licence.oblivio.eu`** (seule adresse compilée dans l'application ; autorisable seule dans un pare-feu).
- ~~Taille du QR avec la licence v2~~ → **décidé le 2026-10-11** : QR imprimé à 5,5 cm (§3) ; essai d'impression à faire.

## 9. Documents mis à jour

**Le 2026-10-11, à la validation** : `CLAUDE.md` (règle 8, liste des documents), [cadre](cadre_projet.md) §1, §2,
§3, §6, [certificat](certificat.md) §4, [protocole](protocole_ipc.md) (licence), [sécurité](securite.md) §1 à §3,
[conformité](conformite.md), [reprise](reprise.md), `README.md` (liste des documents).

**À l'implémentation** (ils décrivent le produit livré, qui reste hors ligne d'ici là) :
[manuel de l'opérateur](manuel_operateur.md) (activation en ligne, secours hors ligne, indicateur « HORS LIGNE »),
accroche du `README.md` et [identité de marque](disphas%20—%20Identité%20&%20Narration%20de%20Marque.md) :
« 100 % offline » → « 100 % hors ligne pour vos données ; seule la licence se connecte ».

## 10. Lot de travail (après validation, dans cet ordre)

1. ~~Mesurer la marge du QR~~ → **fait le 2026-10-11** (§3) ; QR imprimé à 5,5 cm.
2. ~~`disphas-attestation` (public) : certificat d'émetteur, licence v2, révocation, vérification ; tests, fuzzing~~ →
   **fait le 2026-10-11** (`Issuer`, `verify_license`, `is_revoked` ; 10 tests ; cible de fuzzing `attestation`).
   L'application, `disphas-verify` et le rendu PDF vérifient les certificats sous licence v2.
3. ~~`disphas-license` : installation et état des licences v2~~ → **fait le 2026-10-11** : `decode_license` passe par
   `verify_license` ; `InstalledLicense.issuer` ; l'état `Revoked` tient compte de l'émetteur ; une licence installée
   remplace la précédente (renouvellement, même `id`). Le helper installe une v2 et signe avec (`InstallLicense`
   inchangé). Tests : installation, renouvellement, émetteur non certifié, durée hors limites, émetteur révoqué.
4. ~~`disphas-issuer` : signer un certificat d'émetteur, révoquer un émetteur~~ → **fait le 2026-10-11** :
   `disphas-issuer certify ROOT.key --id ISS-… --shop-key HEX --offers A,B --days N --max-days N [--from EPOCH]
   [--register FILE]` imprime le certificat `DSI1:…` à installer sur le serveur et l'inscrit au registre
   `issuers.csv` (à côté de la clé racine ; identifiant unique). La clé boutique est générée sur le serveur ; seule sa
   clé publique (64 chiffres hexadécimaux) vient sur la machine hors ligne ; elle ne peut pas être la racine.
   `revoke --id ISS-…` vérifie l'identifiant dans `issuers.csv` ; `issue` refuse un identifiant `ISS-`.
   `disphas-license` : `issuer_text`, `decode_issuer` (signature par la racine). Cérémonie type :
   `certify --id ISS-2027-01 --offers Atelier,Pro --days 400 --max-days 90` (1 an + chevauchement).
5. ~~Service de signature (`disphas-signer`, §6.2 à §6.5)~~ → **fait le 2026-10-11** (`tools/disphas-signer`) :
   commandes `keygen`, `install`, `status`, `serve` ; requêtes `Activate`, `Renew`, `Status` ; règles du §6.4
   (débit 30/min et 2 000/jour, ce dernier recompté depuis le registre au redémarrage) ; chaque licence est revérifiée
   comme le fera le poste, puis inscrite au registre avant d'être renvoyée. Un certificat installé est pris en compte
   au **redémarrage** du service. Unité systemd durcie `tools/disphas-signer/disphas-signer.service` (vérifiée par
   `systemd-analyze verify`, pas encore lancée sous systemd). Tests : 11 unitaires, et essai du binaire de bout en
   bout avec la racine de développement (socket, activation, renouvellement, refus, installation sur un poste).
   Non fait : export quotidien du registre, chiffrement des clés au repos (à trancher avec l'hébergement).
   L'API publique (§6.6) relève du site (étape 7).
6. Application : activation et renouvellement en ligne, réglage, rappel d'échéance.
7. Site : espace client, sièges, Stripe (document d'architecture du site à part).
