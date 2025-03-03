#!/usr/bin/env bash
set -eou pipefail

NAMESPACE="openebs"
APP_NAME="api-rest"
CERT_SECRET_NAME="rest-api-server-cert"
CERT_DIR="$(dirname "$0")/certs"

rm -rf "${CERT_DIR}"
mkdir -p "${CERT_DIR}"

# Create a self-signed root CA
echo "Creating a self-signed root CA"
openssl genrsa -out "${CERT_DIR}/ca.key" 4096
openssl req -x509 -new -nodes -key "${CERT_DIR}/ca.key" -sha256 -days 3650 -out "${CERT_DIR}/ca.crt" -subj "/CN=api-rest-ca" -addext "subjectAltName=DNS:${NAMESPACE}-${APP_NAME}-${NAMESPACE}.svc.cluster.local,DNS:${NAMESPACE}-${APP_NAME},DNS:${NAMESPACE}-${APP_NAME}-${NAMESPACE}.svc"

# Create TLS certificate for the API REST
echo "Creating a TLS certificate for the API REST"
openssl genrsa -out "${CERT_DIR}/server.key" 4096
openssl req -new -key "${CERT_DIR}/server.key" -out "${CERT_DIR}/server.csr" -subj "/CN=${NAMESPACE}-${APP_NAME}" -addext "subjectAltName=DNS:${NAMESPACE}-${APP_NAME}-${NAMESPACE}.svc.cluster.local,DNS:${NAMESPACE}-${APP_NAME},DNS:${NAMESPACE}-${APP_NAME}-${NAMESPACE}.svc"
openssl x509 -req -in "${CERT_DIR}/server.csr" -CA "${CERT_DIR}/ca.crt" -CAkey "${CERT_DIR}/ca.key" -CAcreateserial -out "${CERT_DIR}/server.crt" -days 3650 -sha256 -extfile <(printf "subjectAltName=DNS:${NAMESPACE}-${APP_NAME}-${NAMESPACE}.svc.cluster.local,DNS:${NAMESPACE}-${APP_NAME},DNS:${NAMESPACE}-${APP_NAME}-${NAMESPACE}.svc")

# Convert the private key to PKCS#1 format if necessary
echo "Verifying the RSA key format"
if grep -q "BEGIN PRIVATE KEY" "${CERT_DIR}/server.key"; then
  echo "Converting key to RSA format"
  openssl rsa -in "${CERT_DIR}/server.key" -out "${CERT_DIR}/server-rsa.key"
  mv "${CERT_DIR}/server-rsa.key" "${CERT_DIR}/server.key"
else
  echo "Key is already in RSA format"
fi

# Create a Kubernetes secret
echo "Creating a Kubernetes secret"
kubectl create secret generic ${CERT_SECRET_NAME} \
  --from-file=tls.crt="${CERT_DIR}/server.crt" \
  --from-file=tls.key="${CERT_DIR}/server.key" \
  --from-file=ca.crt="${CERT_DIR}/ca.crt" \
  -n ${NAMESPACE}