#!/bin/bash
# deploy/deploy-common.sh — configurația SSH comună scripturilor de deploy (ADR-0008).
# Același VPS Oracle Cloud ARM Ampere și același model ca la oracle.gsl.
#
# Suprascrie prin variabile de mediu:
#   DEPLOY_HOST  alias ssh (din ~/.ssh/config) sau IP     implicit: millionphones
#   DEPLOY_USER  utilizatorul ssh cu sudo                  implicit: ubuntu
#   DEPLOY_KEY   cheia privată                             implicit: ~/.ssh/ssh-key-2026-02-21.key

DEPLOY_HOST="${DEPLOY_HOST:-millionphones}"
REMOTE_USER="${DEPLOY_USER:-ubuntu}"
SSH_KEY="${DEPLOY_KEY:-$HOME/.ssh/ssh-key-2026-02-21.key}"

APP_USER="urban"
SERVICE="urban"
REMOTE_DIR="/opt/urban"                                 # server, public/, data/, .env
BUILD_DIR="/home/$REMOTE_USER/build/urban.monitor"      # sursa și target/ pentru build-ul pe server

SSH="ssh -i $SSH_KEY -o StrictHostKeyChecking=accept-new"
REMOTE="$REMOTE_USER@$DEPLOY_HOST"
