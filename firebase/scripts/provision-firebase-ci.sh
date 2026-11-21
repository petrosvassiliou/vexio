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

echo "==> [4/4] Provisioning Keyless Workload Identity Federation (OIDC) for GitHub Actions..."
if command -v gcloud >/dev/null 2>&1; then
  PROJECT_NUM="$(gcloud projects describe "${PROJECT_ID}" --format='value(projectNumber)')"
  SA_NAME="github-action-vexio-pwa"
  SA_EMAIL="${SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"
  POOL_ID="github-actions-pool"
  PROVIDER_ID="github-oidc"

  gcloud iam service-accounts describe "${SA_EMAIL}" --project "${PROJECT_ID}" >/dev/null 2>&1 || \
    gcloud iam service-accounts create "${SA_NAME}" \
      --display-name="GitHub Actions Vexio ChromeOS PWA Deployer" \
      --project "${PROJECT_ID}"

  for ROLE in roles/firebasehosting.admin roles/firebaserules.admin roles/datastore.indexAdmin roles/firebase.viewer roles/serviceusage.serviceUsageConsumer; do
    gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
      --member="serviceAccount:${SA_EMAIL}" \
      --role="${ROLE}" \
      --quiet >/dev/null
  done

  gcloud iam workload-identity-pools describe "${POOL_ID}" --location="global" --project="${PROJECT_ID}" >/dev/null 2>&1 || \
    gcloud iam workload-identity-pools create "${POOL_ID}" \
      --location="global" \
      --display-name="GitHub Actions Pool" \
      --project="${PROJECT_ID}"

  gcloud iam workload-identity-pools providers describe "${PROVIDER_ID}" \
    --workload-identity-pool="${POOL_ID}" --location="global" --project="${PROJECT_ID}" >/dev/null 2>&1 || \
    gcloud iam workload-identity-pools providers create-oidc "${PROVIDER_ID}" \
      --workload-identity-pool="${POOL_ID}" \
      --location="global" \
      --issuer-uri="https://token.actions.githubusercontent.com" \
      --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" \
      --attribute-condition="assertion.repository == '${GH_REPO}'" \
      --project="${PROJECT_ID}"

  gcloud iam service-accounts add-iam-policy-binding "${SA_EMAIL}" \
    --project="${PROJECT_ID}" \
    --role="roles/iam.workloadIdentityUser" \
    --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUM}/locations/global/workloadIdentityPools/${POOL_ID}/attribute.repository/${GH_REPO}" \
    --quiet >/dev/null

  WIF_PROVIDER="projects/${PROJECT_NUM}/locations/global/workloadIdentityPools/${POOL_ID}/providers/${PROVIDER_ID}"
  gh variable set GCP_WORKLOAD_IDENTITY_PROVIDER --body "${WIF_PROVIDER}" -R "${GH_REPO}"
  gh variable set GCP_SERVICE_ACCOUNT_EMAIL --body "${SA_EMAIL}" -R "${GH_REPO}"
  echo "  ✓ Configured keyless OIDC Workload Identity Federation for ${GH_REPO}."
else
  echo "  ! gcloud CLI not detected; Workload Identity Federation variables already configured on ${GH_REPO}."
fi

echo "==> Triggering live Firebase ChromeOS PWA deployment workflow on ${GH_REPO}..."
gh workflow run deploy-firebase-chromeos-pwa.yml -R "${GH_REPO}" --ref main
echo "Done! Monitor deployment at: https://github.com/${GH_REPO}/actions"
