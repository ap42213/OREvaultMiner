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

echo "8. Copying application files..."
# In production, you would clone from git or copy from CI/CD
# git clone https://github.com/your-repo/orevault.git $INSTALL_DIR

echo "9. Building backend..."
cd $INSTALL_DIR/backend
sudo -u $USER cargo build --release

echo "10. Running database migrations..."
sudo -u $USER ./target/release/orevault migrate

echo "11. Installing systemd service..."
cp $INSTALL_DIR/deploy/orevault.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable orevault

echo "12. Starting service..."
systemctl start orevault

echo "13. Checking service status..."
systemctl status orevault --no-pager

echo ""
echo "==================================="
echo "Deployment Complete!"
echo "==================================="
echo ""
echo "Next steps:"
echo "1. Configure .env file: $INSTALL_DIR/backend/.env"
echo "2. Set up Helius RPC API key"
echo "3. Configure firewall (ufw allow 3001)"
echo "4. Set up SSL with nginx (optional)"
echo "5. Deploy frontend to Vercel"
echo ""
echo "View logs: journalctl -u orevault -f"
echo ""
