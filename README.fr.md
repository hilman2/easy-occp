# easy-occp

🌐 [English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md)

[![CI](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml/badge.svg)](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/hilman2/easy-occp)](https://github.com/hilman2/easy-occp/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Outil de gestion simple pour bornes de recharge (wallbox), conçu pour les **PME disposant de 1 à 10 wallbox**.

- **Un seul binaire, un seul fichier SQLite** – pas de base de données externe, pas de message broker.
- **OCPP 1.6J** (complet) + **OCPP 2.0.1** (ossature WebSocket, BootNotification / TransactionEvent) + **OCPP 1.5 SOAP** (ossature pour Boot/Heartbeat).
- **Interface web moderne** (Askama + htmx), utilisateurs locaux avec mots de passe Argon2.
- Active Directory (LDAP) et Microsoft Entra (OIDC) sont préparés sous forme de champs de configuration – les implémentations concrètes des bind/flows suivront.
- Fonctionne sous **Windows** (priorité) **et Linux**.

## Fonctionnalités

| Domaine         | Statut |
|-----------------|--------|
| Inventaire des wallbox + statut (en ligne/hors ligne, connecteurs, firmware) | ✅ |
| Gestion des badges RFID, apprentissage via « fenêtre d'apprentissage » (2 min, interception Authorize) | ✅ |
| Attribution badge → employé, badges invités, expiration de validité | ✅ |
| Déverrouillage à distance pour les invités, imputation sur un label invité | ✅ |
| Valeurs en direct pendant la charge (kWh chargés, puissance actuelle, SoC) | ✅ |
| Liste des transactions, filtre par utilisateur | ✅ |
| Statistiques par mois / trimestre / année | ✅ |
| Gestion des utilisateurs (admin/user), mot de passe local | ✅ |
| Interface multilingue (Deutsch, English, Français, Español) | ✅ |
| Active Directory (LDAP) | 🟡 Config préparée |
| Entra ID (OIDC)        | 🟡 Config préparée |

## Démarrage

**Des binaires prêts à l'emploi** (Windows x64, Linux x64) sont disponibles sous
[Releases](https://github.com/hilman2/easy-occp/releases/latest) — décompresser,
lancer `easy-occp.exe` ou `easy-occp`, c'est tout (voir `INSTALL.fr.md` dans le paquet).

Ou compiler soi-même :

```bash
# une seule fois : créer la config (optionnel)
copy config.example.toml config.toml

# Build + démarrage
cargo run --release
```

Ouvrir ensuite l'interface sur <http://localhost:8080>. **Identifiants par défaut au premier démarrage :** `admin` / `admin` – merci de changer le mot de passe immédiatement sous « Utilisateurs ».

Mot de passe admin oublié ?

```bash
cargo run --release -- --reset-admin "nouveauMotDePasse123"
```

## Configurer une wallbox

Les wallbox établissent une connexion WebSocket :

```
ws://<host>:8080/ocpp/<ChargePointId>
```

Les sous-protocoles sont négociés automatiquement (`ocpp1.6` ou `ocpp2.0.1`).

Pour les appareils OCPP 1.5 plus anciens (SOAP) :

```
POST http://<host>:8080/ocpp15
```

### Mesures en direct pendant la charge

Lors de la connexion d'une wallbox OCPP 1.6, le serveur la configure
automatiquement pour qu'elle transmette toutes les 30 secondes, pendant une
charge, le relevé du compteur, la puissance et (si disponible) le SoC
(`MeterValueSampleInterval`, `MeterValuesSampledData`).
L'intervalle est réglable via `config.toml`, `0` désactive
l'auto-configuration :

```toml
[ocpp]
meter_interval_s = 30
```

Le cockpit et la page de détail de la wallbox actualisent automatiquement les
charges en cours toutes les 10 secondes (polling htmx). Si une wallbox ne
transmet pas la puissance, celle-ci est dérivée des deux derniers relevés de
compteur.

## Stockage des données

Tout est stocké dans **un seul fichier SQLite** sous `data/easy-occp.db` (modifiable via `config.toml`). Les migrations se trouvent dans `migrations/` et sont appliquées automatiquement au démarrage.

### Contrôles de cohérence à la réception des données

- **Timestamps** : >24 h dans le futur ou >10 ans dans le passé sont rejetés – repli sur l'horloge du serveur.
- **StartTransaction / StopTransaction** : idempotents face aux répétitions ; les valeurs de compteur décroissantes sont corrigées.
- **StatusNotification** : UPSERT par (wallbox, connecteur) – pas de doublons.
- **MeterValues** : les valeurs négatives sont rejetées, le SoC est validé entre 0 et 100 %, kWh → Wh normalisés.
- **Enrollment** : un tag nouvellement capturé est associé à exactement une session de fenêtre d'apprentissage ouverte.

## Structure du projet

```
src/
  main.rs           – point d'entrée, runtime Tokio, pool SQLite
  config.rs         – configuration TOML
  db.rs             – bootstrap + helpers (Argon2, paramètres)
  error.rs          – AppError / IntoResponse
  auth/             – cookies de session + login local
  domain/           – modèles de données (FromRow)
  ocpp/
    wire.rs         – parseur de trames JSON OCPP
    hub.rs          – registre de toutes les connexions actives
    ocpp16.rs       – OCPP 1.6J (complet)
    ocpp20.rs       – OCPP 2.0.1 (bootstrap)
    soap15.rs       – endpoint OCPP 1.5 SOAP
  web/              – routeur axum, vues Askama
templates/          – templates HTML (Askama)
static/             – CSS + shim htmx (embarqué via rust-embed)
migrations/         – migrations SQLite
```

## Licence

MIT
