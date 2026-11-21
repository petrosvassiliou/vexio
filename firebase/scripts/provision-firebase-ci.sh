#!/usr/bin/env bash
# ==============================================================================
# Vexio v0.1.0 — Turnkey Production Firebase Hosting, Rules & GitHub CI Linker
#
# Run locally once (after `firebase login` and `gh auth login`) to:
# 1. Create or attach a live Firebase project (e.g., `vexio-chromeos-suite`).
# 2. Fetch the Firebase Web App SDK configuration and store `VITE_FIREBASE_*`
#    repository variables on GitHub (`petrosvassiliou/vexio`).
# 3. Provision a least-privilege Firebase Hosting + Rules Service Account JSON
#    and pipe it directly into `gh secret set FIREBASE_SERVICE_ACCOUNT_VEXIO_CHROMEOS_SUITE`
#    without ever writing long-lived keys to git or chat logs.
# ==============================================================================
set -euo pipefail

PROJECT_ID="${1:-vexio-chromeos-suite}"
GH_REPO="${2:-petrosvassiliou/vexio}"

echo "==> [1/4] Verifying Firebase CLI & GitHub CLI authentication..."
firebase projects:list >/dev/null
gh auth status >/dev/null

echo "==> [2/4] Updating firebase/.firebaserc default project to '${PROJECT_ID}'..."
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cat > "${SCRIPT_DIR}/../.firebaserc" <<EOF
{
  "projects": {
    "default": "${PROJECT_ID}"
  }
}
EOF

echo "==> [3/4] Setting GitHub Actions repository variable FIREBASE_PROJECT_ID=${PROJECT_ID}..."
gh variable set FIREBASE_PROJECT_ID --body "${PROJECT_ID}" -R "${GH_REPO}"

WEB_CONFIG_JSON="$(firebase apps:sdkconfig WEB --project "${PROJECT_ID}" --json 2>/dev/null || true)"
if [[ -n "${WEB_CONFIG_JSON}" ]] && command -v jq >/dev/null 2>&1; then
  API_KEY="$(printf '%s' "${WEB_CONFIG_JSON}" | jq -r '.result.sdkConfig.apiKey // empty')"
  AUTH_DOMAIN="$(printf '%s' "${WEB_CONFIG_JSON}" | jq -r '.result.sdkConfig.authDomain // empty')"
  STORAGE_BUCKET="$(printf '%s' "${WEB_CONFIG_JSON}" | jq -r '.result.sdkConfig.storageBucket // empty')"
  APP_ID="$(printf '%s' "${WEB_CONFIG_JSON}" | jq -r '.result.sdkConfig.appId // empty')"

  [[ -n "${API_KEY}" ]] && gh variable set VITE_FIREBASE_API_KEY --body "${API_KEY}" -R "${GH_REPO}"
  [[ -n "${AUTH_DOMAIN}" ]] && gh variable set VITE_FIREBASE_AUTH_DOMAIN --body "${AUTH_DOMAIN}" -R "${GH_REPO}"
  [[ -n "${STORAGE_BUCKET}" ]] && gh variable set VITE_FIREBASE_STORAGE_BUCKET --body "${STORAGE_BUCKET}" -R "${GH_REPO}"
  [[ -n "${APP_ID}" ]] && gh variable set VITE_FIREBASE_APP_ID --body "${APP_ID}" -R "${GH_REPO}"
  echo "  ✓ Synced VITE_FIREBASE_* SDK variables to ${GH_REPO}."
fi

echo "==> [4/4] Provisioning GitHub Actions Hosting + Rules Service Account secret..."
if command -v gcloud >/dev/null 2>&1; then
  SA_NAME="github-action-vexio-pwa"
  SA_EMAIL="${SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"
  gcloud iam service-accounts describe "${SA_EMAIL}" --project "${PROJECT_ID}" >/dev/null 2>&1 || \
    gcloud iam service-accounts create "${SA_NAME}" \
      --display-name="Vexio ChromeOS PWA GitHub Actions Deployer" \
      --project "${PROJECT_ID}"

  for ROLE in roles/firebasehosting.admin roles/firebaserules.admin roles/datastore.indexAdmin; do
    gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
      --member="serviceAccount:${SA_EMAIL}" \
      --role="${ROLE}" \
      --quiet >/dev/null
  done

  TMP_KEY="$(mktemp)"
  trap 'rm -f "${TMP_KEY}"' EXIT
  gcloud iam service-accounts keys create "${TMP_KEY}" \
    --iam-account="${SA_EMAIL}" \
    --project "${PROJECT_ID}"
  gh secret set FIREBASE_SERVICE_ACCOUNT_VEXIO_CHROMEOS_SUITE -R "${GH_REPO}" < "${TMP_KEY}"
  echo "  ✓ Uploaded FIREBASE_SERVICE_ACCOUNT_VEXIO_CHROMEOS_SUITE secret to ${GH_REPO}."
else
  echo "  ! gcloud CLI not detected; run 'firebase init hosting:github' inside firebase/ to upload FIREBASE_SERVICE_ACCOUNT_VEXIO_CHROMEOS_SUITE."
fi

echo "==> Triggering live Firebase ChromeOS PWA deployment workflow on ${GH_REPO}..."
gh workflow run deploy-firebase-chromeos-pwa.yml -R "${GH_REPO}" --ref main
echo "Done! Monitor deployment at: https://github.com/${GH_REPO}/actions"
