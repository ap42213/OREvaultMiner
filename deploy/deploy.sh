#!/bin/bash
# OreVault Deployment Script for Vultr VPS (NYC)
# 
# Prerequisites:
# - Ubuntu 22.04+ VPS
# - SSH access
# - Domain (optional, for SSL)

set -e

echo "==================================="
echo "OreVault Deployment Script"
echo "==================================="

# Variables
INSTALL_DIR="/opt/orevault"
USER="orevault"
REPO_URL="https://github.com/ap42213/OREvaultMiner.git"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (sudo)"
  exit 1
fi

echo "1. Updating system..."
apt update && apt upgrade -y

echo "2. Installing dependencies..."
apt install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  curl \
  git \
  postgresql \
  postgresql-contrib

echo "3. Installing Rust..."
if ! command -v rustc &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source $HOME/.cargo/env
fi

echo "4. Installing Node.js 20..."
if ! command -v node &> /dev/null; then
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  apt install -y nodejs
fi

echo "5. Creating orevault user..."
if ! id "$USER" &>/dev/null; then
  useradd -m -s /bin/bash $USER
fi

echo "6. Creating installation directory..."
mkdir -p $INSTALL_DIR
chown -R $USER:$USER $INSTALL_DIR

echo "7. Setting up PostgreSQL..."
sudo -u postgres psql -c "CREATE USER orevault WITH PASSWORD 'change_this_password';" 2>/dev/null || true
sudo -u postgres psql -c "CREATE DATABASE orevault OWNER orevault;" 2>/dev/null || true

echo "8. Fetching application code..."
if [ ! -d "$INSTALL_DIR/.git" ]; then
  rm -rf "$INSTALL_DIR"
  sudo -u "$USER" git clone "$REPO_URL" "$INSTALL_DIR"
else
  cd "$INSTALL_DIR"
  sudo -u "$USER" git fetch --all
  sudo -u "$USER" git reset --hard origin/main
fi

echo "9. Ensuring backend env file exists..."
if [ ! -f "$INSTALL_DIR/backend/.env" ]; then
  cp "$INSTALL_DIR/backend/.env.example" "$INSTALL_DIR/backend/.env"
  chown "$USER:$USER" "$INSTALL_DIR/backend/.env"
  echo "Created $INSTALL_DIR/backend/.env from .env.example - EDIT THIS BEFORE STARTING."
fi

echo "10. Installing Rust for orevault user (if needed)..."
sudo -u "$USER" bash -lc 'command -v cargo >/dev/null 2>&1 || (curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y)'

echo "11. Building backend (release)..."
cd $INSTALL_DIR/backend
sudo -u "$USER" bash -lc 'cd /opt/orevault/backend && ~/.cargo/bin/cargo build --release'

echo "NOTE: DB migrations run automatically on backend startup (sqlx::migrate!)."

echo "12. Installing systemd service..."
cp $INSTALL_DIR/deploy/orevault.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable orevault

echo "13. Restarting service..."
systemctl restart orevault

echo "14. Checking service status..."
systemctl status orevault --no-pager

echo ""
echo "==================================="
echo "Deployment Complete!"
echo "==================================="
echo ""
echo "Next steps:"
echo "1. Configure .env file: $INSTALL_DIR/backend/.env (DATABASE_URL, RPC_URL, etc.)"
echo "2. Configure firewall (ufw allow 3001)"
echo "3. (Recommended) Put nginx in front + SSL"
echo "4. Deploy frontend to Vercel (set NEXT_PUBLIC_API_URL + NEXT_PUBLIC_WS_URL)"
echo ""
echo "View logs: journalctl -u orevault -f"
echo ""
