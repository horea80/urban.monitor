#!/bin/bash
# deploy/deploy-common.sh — configurația SSH comună scripturilor de deploy (ADR-0009, ADR-0010).
# Ținta implicită e box-ul împrumutat (Oracle Cloud ARM Ampere, Ubuntu 24.04, nginx pe 80/443): aliasul
# `emailbox` din ~/.ssh/config, cu cheia lui. Același model de deploy ca la oracle.gsl.
#
# Suprascrie prin variabile de mediu:
#   DEPLOY_HOST  alias ssh (din ~/.ssh/config) sau IP     implicit: emailbox
#   DEPLOY_USER  utilizatorul ssh cu sudo                  implicit: ubuntu
#   DEPLOY_KEY   cheia privată                             implicit: ~/.ssh/ssh-key-2026-06-19-rh.key
# VPS-ul propriu (Caddy): DEPLOY_HOST=millionphones DEPLOY_KEY=~/.ssh/ssh-key-2026-02-21.key ./deploy/deploy.sh

DEPLOY_HOST="${DEPLOY_HOST:-emailbox}"
REMOTE_USER="${DEPLOY_USER:-ubuntu}"
SSH_KEY="${DEPLOY_KEY:-$HOME/.ssh/ssh-key-2026-06-19-rh.key}"

APP_USER="urban"
SERVICE="urban"
REMOTE_DIR="/opt/urban"                                 # server, public/, data/, .env
STAGE_DIR="/home/$REMOTE_USER/deploy/urban"             # artefactele proaspăt trimise, înainte de instalare

SSH="ssh -i $SSH_KEY -o StrictHostKeyChecking=accept-new"
REMOTE="$REMOTE_USER@$DEPLOY_HOST"
