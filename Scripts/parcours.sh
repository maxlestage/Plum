#!/usr/bin/env bash
#
# Le parcours complet contre un serveur qui tourne, comme le ferait le client
# iOS : une vraie socket, de vrais en-têtes, un vrai processus.
#
# Les tests d'intégration prouvent la logique, en passant par la couche Router
# en mémoire. Celui-ci prouve l'assemblage — le binaire, la configuration, le
# port, la base — ce qu'aucun d'eux ne touche.
#
#   DATABASE_URL=… JWT_SECRET=… PORT=8099 ./server/target/release/plum-server &
#   PLUM_API="${PLUM_API:-http://127.0.0.1:8099/api/v1}" Scripts/parcours.sh
set -uo pipefail
API="${PLUM_API:-http://127.0.0.1:8099/api/v1}"
fail=0
ok()   { printf "  ok   %s\n" "$1"; }
bad()  { printf "FAIL   %s — %s\n" "$1" "$2"; fail=1; }
eq()   { [ "$2" = "$3" ] && ok "$1" || bad "$1" "attendu $3, reçu $2"; }

EMAIL="parcours-$(date +%s%N)@plum.app"

# 1. Inscription
body=$(curl -s -X POST "$API/auth/sign-up" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"motdepasse\",\"display_name\":\"Camille\",\"birth_date\":\"1996-04-12T00:00:00Z\",\"gender\":\"nonBinary\"}")
ACCESS=$(echo "$body"  | python3 -c 'import sys,json;print(json.load(sys.stdin)["tokens"]["access_token"])' 2>/dev/null)
REFRESH=$(echo "$body" | python3 -c 'import sys,json;print(json.load(sys.stdin)["tokens"]["refresh_token"])' 2>/dev/null)
[ -n "${ACCESS:-}" ] && ok "inscription" || bad "inscription" "$body"

AUTH=(-H "authorization: Bearer $ACCESS")

# 2. Le compte
eq "/me" "$(curl -s -o /dev/null -w '%{http_code}' "$API/me" "${AUTH[@]}")" 200
eq "/me sans jeton" "$(curl -s -o /dev/null -w '%{http_code}' "$API/me")" 401

# 3. Le profil créé par l'inscription
p=$(curl -s "$API/me/profile" "${AUTH[@]}")
eq "profil : prénom" "$(echo "$p" | python3 -c 'import sys,json;print(json.load(sys.stdin)["display_name"])')" "Camille"
eq "profil : genre sur le fil" "$(echo "$p" | python3 -c 'import sys,json;print(json.load(sys.stdin)["gender"])')" "nonBinary"
eq "profil : photos présent et vide" "$(echo "$p" | python3 -c 'import sys,json;print(json.dumps(json.load(sys.stdin)["photos"]))')" "[]"

# 4. Modification partielle
curl -s -o /dev/null -X PATCH "$API/me/profile" "${AUTH[@]}" -H 'content-type: application/json' \
  -d '{"bio":"J’aime les prunes.","city":"Lyon","interests":["  Cinéma ","cinéma","Randonnée"]}'
p=$(curl -s -X PATCH "$API/me/profile" "${AUTH[@]}" -H 'content-type: application/json' -d '{"city":"Marseille"}')
eq "ville modifiée" "$(echo "$p" | python3 -c 'import sys,json;print(json.load(sys.stdin)["city"])')" "Marseille"
eq "bio préservée"  "$(echo "$p" | python3 -c 'import sys,json;print(json.load(sys.stdin)["bio"])')" "J’aime les prunes."
eq "intérêts rangés" "$(echo "$p" | python3 -c 'import sys,json;print(json.dumps(json.load(sys.stdin)["interests"],ensure_ascii=False))')" '["Cinéma", "Randonnée"]'

# 5. Préférences : défauts puis bornes serveur
eq "préférences par défaut" "$(curl -s "$API/me/preferences" "${AUTH[@]}" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["interested_in"],d["min_age"],d["max_age"],d["max_distance_km"])')" "everyone 18 45 50"
pr=$(curl -s -X PATCH "$API/me/preferences" "${AUTH[@]}" -H 'content-type: application/json' \
  -d '{"interested_in":"women","min_age":13,"max_age":4,"max_distance_km":99999,"show_me_on_plum":false}')
eq "bornes appliquées" "$(echo "$pr" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["min_age"],d["max_age"],d["max_distance_km"])')" "18 18 300"

# 6. Position
eq "position refusée hors bornes" "$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$API/me/location" "${AUTH[@]}" -H 'content-type: application/json' -d '{"latitude":91,"longitude":2}')" 400
eq "position acceptée" "$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$API/me/location" "${AUTH[@]}" -H 'content-type: application/json' -d '{"latitude":48.8566,"longitude":2.3522}')" 200

# 7. Fin d'accueil, rejouable
eq "complete"          "$(curl -s -X POST "$API/me/profile/complete" "${AUTH[@]}" | python3 -c 'import sys,json;print(json.load(sys.stdin)["profile_completed"])')" "True"
eq "complete rejoué"   "$(curl -s -X POST "$API/me/profile/complete" "${AUTH[@]}" | python3 -c 'import sys,json;print(json.load(sys.stdin)["profile_completed"])')" "True"

# 8. Rotation du jeton de rafraîchissement
new=$(curl -s -X POST "$API/auth/refresh" -H 'content-type: application/json' -d "{\"refresh_token\":\"$REFRESH\"}")
NEWREFRESH=$(echo "$new" | python3 -c 'import sys,json;print(json.load(sys.stdin)["refresh_token"])' 2>/dev/null)
[ -n "${NEWREFRESH:-}" ] && ok "rafraîchissement" || bad "rafraîchissement" "$new"
eq "ancien jeton refusé" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/auth/refresh" -H 'content-type: application/json' -d "{\"refresh_token\":\"$REFRESH\"}")" 401

# 9. Énumération des comptes fermée : inconnu et mauvais mot de passe identiques
a=$(curl -s -X POST "$API/auth/sign-in" -H 'content-type: application/json' -d "{\"email\":\"inconnu-$(date +%s%N)@plum.app\",\"password\":\"x\"}")
b=$(curl -s -X POST "$API/auth/sign-in" -H 'content-type: application/json' -d "{\"email\":\"$EMAIL\",\"password\":\"mauvais\"}")
eq "réponses identiques" "$a" "$b"

# 10. Le deck, les verdicts et le retour en arrière
autre=$(curl -s -X POST "$API/auth/sign-up" -H 'content-type: application/json' \
  -d "{\"email\":\"parcours-autre-$(date +%s%N)@plum.app\",\"password\":\"motdepasse\",\"display_name\":\"Dominique\",\"birth_date\":\"1996-04-12T00:00:00Z\",\"gender\":\"woman\"}")
AUTRE_ID=$(echo "$autre" | python3 -c 'import sys,json;print(json.load(sys.stdin)["user"]["id"])' 2>/dev/null)
AUTRE_TOKEN=$(echo "$autre" | python3 -c 'import sys,json;print(json.load(sys.stdin)["tokens"]["access_token"])' 2>/dev/null)
curl -s -o /dev/null -X PATCH "$API/me/location" -H "authorization: Bearer $AUTRE_TOKEN" \
  -H 'content-type: application/json' -d '{"latitude":48.8570,"longitude":2.3525}'

eq "deck accessible" "$(curl -s -o /dev/null -w '%{http_code}' "$API/discovery/deck?limit=5" "${AUTH[@]}")" 200

# Un j'aime réciproque doit annoncer le match au second qui répond.
curl -s -o /dev/null -X POST "$API/discovery/swipes" -H "authorization: Bearer $AUTRE_TOKEN" \
  -H 'content-type: application/json' -d "{\"target_profile_id\":\"$(curl -s "$API/me" "${AUTH[@]}" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')\",\"decision\":\"like\"}"
m=$(curl -s -X POST "$API/discovery/swipes" "${AUTH[@]}" -H 'content-type: application/json' \
  -d "{\"target_profile_id\":\"$AUTRE_ID\",\"decision\":\"superLike\"}")
eq "match réciproque" "$(echo "$m" | python3 -c 'import sys,json;print(json.load(sys.stdin)["matched"])')" "True"
eq "pas de quota inventé" "$(echo "$m" | python3 -c 'import sys,json;print(json.load(sys.stdin)["likes_remaining"])')" "None"

eq "second verdict refusé" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/discovery/swipes" "${AUTH[@]}" -H 'content-type: application/json' -d "{\"target_profile_id\":\"$AUTRE_ID\",\"decision\":\"like\"}")" 400
eq "blocage idempotent" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/profiles/$AUTRE_ID/block" "${AUTH[@]}")$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/profiles/$AUTRE_ID/block" "${AUTH[@]}")" "200200"
eq "signalement sans motif refusé" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/profiles/$AUTRE_ID/report" "${AUTH[@]}" -H 'content-type: application/json' -d '{"reason":"  "}')" 400

# 11. Déconnexion
eq "déconnexion" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/auth/sign-out" "${AUTH[@]}" -H 'content-type: application/json' -d "{\"refresh_token\":\"$NEWREFRESH\"}")" 200
eq "jeton de rafraîchissement mort" "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$API/auth/refresh" -H 'content-type: application/json' -d "{\"refresh_token\":\"$NEWREFRESH\"}")" 401

[ $fail -eq 0 ] && echo -e "\nPARCOURS COMPLET AU VERT" || echo -e "\nÉCHECS"
exit $fail
