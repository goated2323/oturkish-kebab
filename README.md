# O'Turkish Kebab — Backend Rust + Stripe Checkout

Site vitrine one-page + panier + paiement en ligne.

## Structure
- `index.html:1` — frontend statique (HTML/CSS/JS, panier `localStorage`, fetch `/api/checkout`)
- `src/main.rs:1` — backend Rust (Axum, Tokio, tower-http, reqwest)
- `Cargo.toml:1` — dépendances Rust
- `.env` / `.env.example` — configuration
- `images/` — photos produits

## Lancer le backend Rust (recommandé)
```bat
.\start_rust.bat
# ou
cargo run
```
- Serveur : `http://127.0.0.1:3000` (port 3000 car 8080 occupé par Steam CEF)
- `PORT` et `STATIC_DIR` configurables via `.env` (`src/main.rs:187`)
- Statique : `index.html` + `images/` servis par `ServeDir` (`src/main.rs:204`)

### API
- `GET /api/health` → `{"status":"ok"}`
- `GET /api/config` → `{"has_stripe":bool}`
- `POST /api/checkout` → Stripe Checkout Session

**Requête checkout :**
```json
{
  "items": [
    {"name":"Kebab","unit_amount":600,"quantity":2},
    {"name":"Coca-Cola","unit_amount":150,"quantity":1}
  ]
}
```
`unit_amount` en centimes (600 = 6,00€). Réponse `{"url":"https://checkout.stripe.com/..."}` → redirection.

**Sans clé Stripe** : `503` + message → front propose téléphone.
**Avec clé invalide** : `502` + `Stripe: Invalid API Key...`
**Panier vide** : `400`

Frontend appelle `fetch("/api/checkout")` (`index.html:137`) et gère `?success=1` / `?canceled=1`.

## Config Stripe
1. Créer compte https://dashboard.stripe.com/test/apikeys
2. Copier `.env.example` → `.env`
3. Renseigner :
```
STRIPE_SECRET_KEY=sk_test_51...
STRIPE_PUBLISHABLE_KEY=pk_test_51...
STRIPE_SUCCESS_URL=http://127.0.0.1:3000/?success=1
STRIPE_CANCEL_URL=http://127.0.0.1:3000/?canceled=1
```
4. Relancer `cargo run` — tester :
```powershell
Invoke-WebRequest http://127.0.0.1:3000/api/config
Invoke-WebRequest http://127.0.0.1:3000/api/checkout -Method POST -Body '{"items":[{"name":"Kebab","unit_amount":600,"quantity":1}]}' -ContentType "application/json"
```

Implémentation Stripe : `src/main.rs:109` construit `line_items[N][price_data][...]` et fait `POST https://api.stripe.com/v1/checkout/sessions` avec `basic_auth(secret)` (`src/main.rs:132`).

## Ancien serveur Python
`python server.py 8080` reste disponible mais 8080 est occupé par Steam. Préférer Rust sur 3000.

## Frontend
- Panier : `index.html:119` (`otk-cart` localStorage)
- Tabs menu : `index.html:101` (`data` sandwiches/tacos/burgers...)
- Paiement : `index.html:137` (`BACKEND_URL=""`, `checkout()` async)
