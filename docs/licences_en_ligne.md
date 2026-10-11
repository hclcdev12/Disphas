# Oblivio — Licences : émission en ligne, activation et renouvellement

> **Validé par le propriétaire le 2026-10-11. En cours d'implémentation** (§10) : format, vérification et
> installation par le helper de la licence v2, certification de la clé boutique (`oblivio-issuer certify`) faits ;
> service de signature et activation en ligne à faire.
> D'ici là, les licences s'émettent et s'activent comme le décrit le [certificat](certificat.md) §4.
> Référence des licences, avec le [cadre](cadre_projet.md) §3 qui le résume.

## 1. Besoin (exprimé par le propriétaire le 2026-10-11)

1. Le client achète en ligne (Stripe) et obtient sa licence **sans intervention de l'éditeur**.
2. **Un seul copier-coller** : une clé de licence prise dans l'espace client, collée dans les réglages d'Oblivio.
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
| D6 | **Secours hors ligne** conservé : demande `OBA1` collée dans l'espace client, licence recopiée à la main | — (le parcours actuel reste valable) |
| D7 | Licences v1 signées par la racine **toujours acceptées** (grands comptes, postes sans Internet, devis) | — |

**Règle 8 reformulée** (reportée dans `CLAUDE.md` §3) : *Aucun appel réseau ni télémétrie, à une exception près : l'application peut
contacter le serveur de licences d'Oblivio pour activer ou renouveler la licence. Seuls transitent la clé de licence,
la clé publique du poste et l'identifiant de licence. Le helper n'ouvre jamais de connexion. Aucune donnée de disque
ni de certificat ne quitte le poste.*

## 3. Chaîne de confiance

```
Clé racine (hors ligne, éditeur)                     inchangée
   ├──signe──► Licence v1 OBL1 {…, wpk}               émission manuelle (oblivio-issuer), toujours acceptée
   └──signe──► Certificat d'émetteur OBI1             1 par an, cérémonie hors ligne
                  {v: 1, iid, ipk, nbf, exp, off[], maxd}
                     └── clé boutique (serveur) ──signe──► Licence v2 OBL1 {v: 2, id, cli, off, nbf, exp, wpk, iss}
                                                              └── clé du poste ──signe──► Attestation (inchangée)
```

**Certificat d'émetteur** (CBOR déterministe, COSE_Sign1 par la racine ; `oblivio_attestation::Issuer`) : `iid`
identifiant d'émetteur, **préfixe `ISS-` obligatoire**, 64 caractères au plus ; `ipk` clé publique Ed25519 de la
boutique ; `nbf`/`exp` validité, `exp > nbf` (1 an, avec chevauchement pour la rotation) ; `off` offres qu'il peut
émettre, non vide, triées, sans doublon ; `maxd` durée maximale d'une licence en secondes, non nulle (90 jours, le
plafond du §4).

**Licence v2** : les champs de la v1 avec `v: 2`, `exp` obligatoire, plus `iss` = le certificat d'émetteur signé
(octets du COSE_Sign1), **embarqué** pour que le `.lic` et le QR restent vérifiables seuls, hors ligne, avec la seule
clé racine. Une licence `v: 1` ne porte jamais `iss`. Aucun identifiant de licence (v1 ou v2) ne commence par `ISS-`.

**Vérification v2** (`oblivio_attestation::verify_license`, utilisée par `verify`) : certificat d'émetteur signé par
la racine ; licence signée par `ipk` ; `off` ∈ `iss.off` ; `[nbf, exp]` de la licence inclus dans celui de
l'émetteur ; `exp − nbf ≤ maxd` ; ni la licence ni l'émetteur révoqués à la date `iat` de l'attestation
(`is_revoked`). Erreurs rendues avec les codes existants (`License`, `LicenseContent`, `LicenseRevoked`) : les
interfaces de vérification n'ont pas changé.

**Révocation** (simplifiée à l'implémentation, 2026-10-11) : la liste `OBR1` **reste en `v: 1`**. Ses entrées
`[[id, date], …]` désignent une licence **ou** un émetteur : le préfixe `ISS-`, réservé aux émetteurs et interdit aux
licences, lève toute ambiguïté. Mêmes règles (définitive, la date la plus ancienne l'emporte). Révoquer un siège =
révoquer son `id` ; révoquer un émetteur = révoquer son `iid`. `oblivio-issuer revoke` et `oblivio-verify` restent
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
- Avec un identifiant de licence réel (`OBL-AAAAMMJJ-…`), le cas typique v1 est **déjà à 991 caractères** : le
  test actuel (`< 1000`, identifiant fictif court `LIC-0001`) sous-estime, et la cible « < 1 000 » est déjà
  dépassée aujourd'hui par le pire cas v1. La vraie contrainte est la **lisibilité du QR imprimé** (taille d'un module).
- **Décidé le 2026-10-11** : QR imprimé à **5,5 cm** (au lieu de 4,4 cm), pour que le pire cas v2 garde un module
  d'au moins 0,40 mm, celui d'un certificat typique à 4,4 cm. Garde-fou : test
  `largest_certificate_keeps_a_readable_qr_code` d'`oblivio-report` (certificat le plus long, `fixture::signed_largest`),
  qui échoue à 4,4 cm. **Reste à faire** : essai réel, impression du pire cas (`largest-fr.pdf` de l'exemple `sample`)
  et lecture au téléphone (le certificat de l'exemple porte déjà une licence v2).

Repli possible : le QR ne porte que `iid`, et les certificats d'émetteur sont livrés avec le vérificateur
(conséquence : un émetteur créé après la version du vérificateur n'est vérifiable qu'après sa mise à jour).

## 4. Parcours

### Activation (une fois par poste)

1. Espace client : l'abonnement affiche une **clé de licence** (secret aléatoire de 128 bits, en groupes lisibles)
   et le nombre de postes utilisés / autorisés.
2. Oblivio, Réglages → Licence : le client colle la clé, puis **Activer**.
3. L'application demande au helper une demande d'activation `OBA1` (commande existante), l'envoie au serveur avec la clé.
4. Le serveur vérifie la clé, l'abonnement actif et les postes disponibles, fait signer une licence v2 pour ce `wpk`
   et la renvoie. L'application la transmet au helper (`InstallLicense`, commande existante), qui la vérifie comme
   aujourd'hui.

### Renouvellement (automatique)

- Au lancement puis toutes les quelques heures, si la licence expire dans moins de **10 jours** et que le
  renouvellement en ligne est activé, l'application envoie une nouvelle `OBA1` et l'identifiant de licence.
- Abonnement payé → licence prolongée, même `id`, installée sans action du client. Sinon → rien ; la licence expire
  à sa date et Oblivio retombe en diagnostic seul (comportement actuel).
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

Poste sans Internet : le client colle la demande `OBA1` dans l'espace client (Ajouter un poste) et recopie la licence
dans Oblivio, comme aujourd'hui. Les renouvellements se recopient de même depuis l'espace client (le serveur connaît
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
- Le **helper ne change pas** : il ne voit que des textes `OBA1`/`OBL1`, comme aujourd'hui.
- Rappel local de l'échéance (« Votre licence expire dans N jours »), sans réseau.
- L'indicateur « HORS LIGNE » devient exact au mot près : affiché en permanence, sauf pendant une requête de licence.

## 6. Côté serveur

- **Site** (Astro, comptes, Stripe) : abonnements, sièges, clés de licence, postes, factures, téléchargements.
- **Service de signature** (Rust, réutilise `oblivio-license` / `oblivio-attestation`) : seul détenteur de la clé
  boutique, compte système dédié, écoute sur une socket locale uniquement, ne fait que signer dans les bornes du
  certificat d'émetteur, débit limité, inscrit chaque licence au **registre**.
- **Registre** exporté chaque jour hors du serveur (chiffré). Il sert à réémettre les licences légitimes après incident.

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
accroche du `README.md` et [identité de marque](Oblivio%20—%20Identité%20&%20Narration%20de%20Marque.md) :
« 100 % offline » → « 100 % hors ligne pour vos données ; seule la licence se connecte ».

## 10. Lot de travail (après validation, dans cet ordre)

1. ~~Mesurer la marge du QR~~ → **fait le 2026-10-11** (§3) ; QR imprimé à 5,5 cm.
2. ~~`oblivio-attestation` (public) : certificat d'émetteur, licence v2, révocation, vérification ; tests, fuzzing~~ →
   **fait le 2026-10-11** (`Issuer`, `verify_license`, `is_revoked` ; 10 tests ; cible de fuzzing `attestation`).
   L'application, `oblivio-verify` et le rendu PDF vérifient les certificats sous licence v2.
3. ~~`oblivio-license` : installation et état des licences v2~~ → **fait le 2026-10-11** : `decode_license` passe par
   `verify_license` ; `InstalledLicense.issuer` ; l'état `Revoked` tient compte de l'émetteur ; une licence installée
   remplace la précédente (renouvellement, même `id`). Le helper installe une v2 et signe avec (`InstallLicense`
   inchangé). Tests : installation, renouvellement, émetteur non certifié, durée hors limites, émetteur révoqué.
4. ~~`oblivio-issuer` : signer un certificat d'émetteur, révoquer un émetteur~~ → **fait le 2026-10-11** :
   `oblivio-issuer certify ROOT.key --id ISS-… --shop-key HEX --offers A,B --days N --max-days N [--from EPOCH]
   [--register FILE]` imprime le certificat `OBI1:…` à installer sur le serveur et l'inscrit au registre
   `issuers.csv` (à côté de la clé racine ; identifiant unique). La clé boutique est générée sur le serveur ; seule sa
   clé publique (64 chiffres hexadécimaux) vient sur la machine hors ligne ; elle ne peut pas être la racine.
   `revoke --id ISS-…` vérifie l'identifiant dans `issuers.csv` ; `issue` refuse un identifiant `ISS-`.
   `oblivio-license` : `issuer_text`, `decode_issuer` (signature par la racine). Cérémonie type :
   `certify --id ISS-2027-01 --offers Atelier,Pro --days 400 --max-days 90` (1 an + chevauchement).
5. Service de signature.
6. Application : activation et renouvellement en ligne, réglage, rappel d'échéance.
7. Site : espace client, sièges, Stripe (document d'architecture du site à part).
