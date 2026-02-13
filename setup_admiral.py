#!/usr/bin/env python3
"""
Setup script for Envyro-Core
Creates Admiral account with a secure password.
"""

import sys
import os
import getpass
import psycopg2
from psycopg2 import sql

try:
    import bcrypt
except ImportError:
    print("Error: bcrypt not installed. Install with: pip install bcrypt")
    sys.exit(1)


def get_db_config():
    """Get database configuration from environment or prompt."""
    config = {
        'host': os.getenv('ENVYRO_DB_HOST') or input("Database host [localhost]: ").strip() or 'localhost',
        'port': int(os.getenv('ENVYRO_DB_PORT') or input("Database port [5432]: ").strip() or '5432'),
        'database': os.getenv('ENVYRO_DB_NAME') or input("Database name [envyro]: ").strip() or 'envyro',
        'user': os.getenv('ENVYRO_DB_USER') or input("Database user [postgres]: ").strip() or 'postgres',
        'password': os.getenv('ENVYRO_DB_PASSWORD') or getpass.getpass("Database password: ")
    }
    return config


def create_admiral(connection, username, password):
    """Create Admiral account with bcrypt-hashed password."""
    # Get bcrypt cost factor from environment or use default
    cost_factor = int(os.getenv('BCRYPT_COST_FACTOR', '12'))
    
    # Hash the password
    password_hash = bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt(cost_factor))
    
    try:
        with connection.cursor() as cursor:
            cursor.execute("""
                INSERT INTO users (username, password_hash, role)
                VALUES (%s, %s, 'admiral')
                ON CONFLICT (username) DO UPDATE
                SET password_hash = EXCLUDED.password_hash
            """, (username, password_hash.decode('utf-8')))
            
            connection.commit()
            print(f"✓ Admiral account '{username}' created/updated successfully!")
            return True
    except Exception as e:
        connection.rollback()
        print(f"✗ Error creating Admiral account: {e}")
        return False


def main():
    """Main setup function."""
    print("=" * 60)
    print("Envyro-Core Setup: Admiral Account Creation")
    print("=" * 60)
    print()
    
    print("This script will create or update the Admiral account.")
    print("The Admiral has complete 'God Mode' over the system.")
    print()
    
    # Get database configuration
    print("Database Configuration")
    print("-" * 60)
    db_config = get_db_config()
    print()
    
    # Connect to database
    print("Connecting to database...")
    try:
        connection = psycopg2.connect(**db_config)
        print("✓ Connected successfully")
        print()
    except Exception as e:
        print(f"✗ Failed to connect to database: {e}")
        print("\nMake sure:")
        print("  1. PostgreSQL is running")
        print("  2. The database exists (run: createdb envyro)")
        print("  3. The schema is initialized (run: psql -d envyro -f init_db.sql)")
        sys.exit(1)
    
    # Get Admiral credentials
    print("Admiral Account")
    print("-" * 60)
    username = input("Admiral username [admin]: ").strip() or 'admin'
    
    while True:
        password = getpass.getpass("Admiral password: ")
        if len(password) < 8:
            print("✗ Password must be at least 8 characters long")
            continue
        
        password_confirm = getpass.getpass("Confirm password: ")
        if password != password_confirm:
            print("✗ Passwords don't match")
            continue
        
        break
    
    print()
    
    # Create Admiral account
    print("Creating Admiral account...")
    success = create_admiral(connection, username, password)
    
    connection.close()
    
    if success:
        print()
        print("=" * 60)
        print("✓ Setup complete!")
        print()
        print("Admiral credentials:")
        print(f"  Username: {username}")
        print(f"  Password: [hidden]")
        print()
        print("Keep these credentials secure!")
        print("=" * 60)
        sys.exit(0)
    else:
        sys.exit(1)


if __name__ == "__main__":
    main()
