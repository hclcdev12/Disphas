# Politique de sécurité — Security policy

> ⚠️ **À compléter avant la première vente** : adresse de contact et délais définitifs
> (voir [docs/conformite.md](docs/conformite.md), action C1).

## Signaler une vulnérabilité

disphas efface des données et signe des certificats : une faille peut avoir des conséquences
graves. Si vous pensez en avoir trouvé une, **merci de nous la signaler en privé** et de ne pas
la publier (ni ticket public, ni forum) avant qu'un correctif soit disponible.

- **Contact** : `security@<domaine à définir>` — *adresse dédiée à créer*.
- **À joindre** : version de disphas (affichée dans l'en-tête de l'application), système, matériel
  concerné (modèle de disque, firmware), étapes pour reproduire, impact constaté. **N'envoyez jamais
  de données réelles** ni de certificat contenant des informations personnelles : un exemple
  reconstitué suffit.
- **Accusé de réception** : sous **5 jours ouvrés** *(délai indicatif, à confirmer)*.
- Nous vous tiendrons informé du diagnostic et du correctif, et vous créditerons si vous le
  souhaitez.

Sont notamment concernés : un effacement déclaré réussi alors que des données subsistent, un
certificat falsifiable ou accepté à tort, une élévation de privilèges via le helper root, une
action sur un disque protégé (système, monté, membre d'un volume).

## Versions prises en charge

disphas n'est pas encore commercialisé. Les correctifs de sécurité porteront sur la dernière
version publiée.

---

## Reporting a vulnerability (English)

Please report suspected vulnerabilities **privately** to `security@<domain to be defined>`
(*dedicated address to be created*) and do not disclose them publicly before a fix is available.
Include the disphas version, system, hardware (drive model, firmware), steps to reproduce and the
observed impact — **never real data** or certificates containing personal information.
We aim to acknowledge reports within **5 business days** (*indicative, to be confirmed*).
