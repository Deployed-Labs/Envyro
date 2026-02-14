#!/usr/bin/env python3
"""
Envyro Security Migration Script
Handles encryption of existing data and database schema updates
"""

import os
import sys
import psycopg2
import psycopg2.extras
from pathlib import Path
import json
from datetime import datetime, timedelta

# Add the envyro_core directory to the path
sys.path.insert(0, str(Path(__file__).parent / 'envyro_core'))

from security import EnvyroSecurity
from secure_config import SecureConfig

class SecurityMigration:
    def __init__(self):
        self.security = EnvyroSecurity()
        self.secure_config = SecureConfig()

        # Database connection parameters
        self.db_config = {
            'host': os.getenv('DB_HOST', 'localhost'),
            'port': os.getenv('DB_PORT', '5432'),
            'database': os.getenv('DB_NAME', 'envyro'),
            'user': os.getenv('DB_USER', 'envyro_user'),
            'password': os.getenv('DB_PASSWORD', 'envyro_pass')
        }

    def get_db_connection(self):
        """Get database connection with proper error handling"""
        try:
            conn = psycopg2.connect(**self.db_config)
            conn.autocommit = False  # We'll manage transactions
            return conn
        except psycopg2.Error as e:
            print(f"Database connection failed: {e}")
            sys.exit(1)

    def backup_existing_data(self, conn):
        """Create backup of existing data before migration"""
        print("Creating backup of existing data...")

        backup_dir = Path("backups")
        backup_dir.mkdir(exist_ok=True)

        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_file = backup_dir / f"envyro_backup_{timestamp}.sql"

        try:
            with conn.cursor() as cursor:
                # Get all table data
                cursor.execute("""
                    SELECT table_name FROM information_schema.tables
                    WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
                    AND table_name NOT LIKE 'pg_%' AND table_name NOT LIKE 'sql_%'
                """)
                tables = cursor.fetchall()

                with open(backup_file, 'w') as f:
                    f.write("-- Envyro Database Backup\n")
                    f.write(f"-- Created: {datetime.now()}\n\n")

                    for (table_name,) in tables:
                        cursor.execute(f"SELECT * FROM {table_name}")
                        rows = cursor.fetchall()

                        if rows:
                            f.write(f"-- Data from table: {table_name}\n")
                            for row in rows:
                                # Simple INSERT generation (not perfect for all data types)
                                escaped_values = []
                                for val in row:
                                    if val is not None:
                                        escaped_val = str(val).replace("'", "''")
                                        escaped_values.append(f"'{escaped_val}'")
                                    else:
                                        escaped_values.append('NULL')
                                values = ', '.join(escaped_values)
                                f.write(f"INSERT INTO {table_name} VALUES ({values});\n")
                            f.write("\n")

            print(f"Backup created: {backup_file}")
            return backup_file

        except Exception as e:
            print(f"Backup failed: {e}")
            return None

    def migrate_users_table(self, conn):
        """Migrate users table to new schema with encryption"""
        print("Migrating users table...")

        with conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cursor:
            # Check if new columns exist
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'users' AND table_schema = 'public'
            """)
            existing_columns = {row['column_name'] for row in cursor.fetchall()}

            # Add new columns if they don't exist
            new_columns = {
                'email': 'TEXT',
                'last_login': 'TIMESTAMP WITH TIME ZONE',
                'is_active': 'BOOLEAN DEFAULT TRUE',
                'failed_login_attempts': 'INTEGER DEFAULT 0',
                'locked_until': 'TIMESTAMP WITH TIME ZONE',
                'preferences': 'JSONB DEFAULT \'{}\''
            }

            for col, col_type in new_columns.items():
                if col not in existing_columns:
                    cursor.execute(f"ALTER TABLE users ADD COLUMN {col} {col_type}")
                    print(f"Added column: {col}")

            # Update role constraint
            cursor.execute("""
                ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
                ALTER TABLE users ADD CONSTRAINT users_role_check
                CHECK (role IN ('admiral', 'user', 'guest'))
            """)

            # Encrypt existing email data if any
            cursor.execute("SELECT id, email FROM users WHERE email IS NOT NULL AND email != ''")
            users_with_email = cursor.fetchall()

            for user in users_with_email:
                encrypted_email = self.security.encrypt_data(user['email'])
                cursor.execute(
                    "UPDATE users SET email = %s WHERE id = %s",
                    (encrypted_email, user['id'])
                )

            print("Users table migration completed")

    def migrate_envyro_knowledge_table(self, conn):
        """Migrate envyro_knowledge table to new schema"""
        print("Migrating envyro_knowledge table...")

        with conn.cursor() as cursor:
            # Check if new columns exist
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'envyro_knowledge' AND table_schema = 'public'
            """)
            existing_columns = {row['column_name'] for row in cursor.fetchall()}

            new_columns = {
                'updated_at': 'TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP',
                'access_level': "TEXT DEFAULT 'public'",
                'metadata': "JSONB DEFAULT '{}'"
            }

            for col, col_type in new_columns.items():
                if col not in existing_columns:
                    cursor.execute(f"ALTER TABLE envyro_knowledge ADD COLUMN {col} {col_type}")
                    print(f"Added column: {col}")

            # Add constraint for access_level
            cursor.execute("""
                ALTER TABLE envyro_knowledge ADD CONSTRAINT envyro_knowledge_access_level_check
                CHECK (access_level IN ('public', 'user', 'admin'))
            """)

            # Create trigger for updated_at
            cursor.execute("""
                DROP TRIGGER IF EXISTS update_envyro_knowledge_updated_at ON envyro_knowledge;
                CREATE TRIGGER update_envyro_knowledge_updated_at
                    BEFORE UPDATE ON envyro_knowledge
                    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
            """)

            print("Envyro knowledge table migration completed")

    def create_new_tables(self, conn):
        """Create new security-related tables"""
        print("Creating new security tables...")

        with conn.cursor() as cursor:
            # User sessions table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS user_sessions (
                    id SERIAL PRIMARY KEY,
                    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
                    session_token TEXT UNIQUE NOT NULL,
                    ip_address INET,
                    user_agent TEXT,
                    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
                    is_active BOOLEAN DEFAULT TRUE
                )
            """)

            # File storage table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS file_storage (
                    id SERIAL PRIMARY KEY,
                    filename TEXT NOT NULL,
                    original_filename TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_size BIGINT,
                    mime_type TEXT,
                    uploaded_by INTEGER REFERENCES users(id),
                    uploaded_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    access_level TEXT DEFAULT 'private' CHECK (access_level IN ('public', 'user', 'admin')),
                    checksum TEXT,
                    metadata JSONB DEFAULT '{}'
                )
            """)

            # Audit log table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS audit_log (
                    id SERIAL PRIMARY KEY,
                    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                    user_id INTEGER REFERENCES users(id),
                    action TEXT NOT NULL,
                    resource_type TEXT NOT NULL,
                    resource_id TEXT,
                    ip_address INET,
                    user_agent TEXT,
                    details JSONB DEFAULT '{}',
                    success BOOLEAN DEFAULT TRUE
                )
            """)

            # Create indexes
            indexes = [
                "CREATE INDEX IF NOT EXISTS user_sessions_token_idx ON user_sessions(session_token)",
                "CREATE INDEX IF NOT EXISTS user_sessions_expires_idx ON user_sessions(expires_at)",
                "CREATE INDEX IF NOT EXISTS file_storage_uploaded_by_idx ON file_storage(uploaded_by)",
                "CREATE INDEX IF NOT EXISTS file_storage_access_level_idx ON file_storage(access_level)",
                "CREATE INDEX IF NOT EXISTS audit_log_timestamp_idx ON audit_log(timestamp)",
                "CREATE INDEX IF NOT EXISTS audit_log_user_idx ON audit_log(user_id)"
            ]

            for index_sql in indexes:
                cursor.execute(index_sql)

            print("New security tables created")

    def create_database_functions(self, conn):
        """Create database functions for security operations"""
        print("Creating database security functions...")

        with conn.cursor() as cursor:
            # Function to increment failed login attempts
            cursor.execute("""
                CREATE OR REPLACE FUNCTION increment_failed_login_attempts(user_id_param INTEGER)
                RETURNS VOID AS $$
                BEGIN
                    UPDATE users
                    SET failed_login_attempts = failed_login_attempts + 1,
                        locked_until = CASE
                            WHEN failed_login_attempts >= 5 THEN CURRENT_TIMESTAMP + INTERVAL '15 minutes'
                            ELSE NULL
                        END
                    WHERE id = user_id_param;
                END;
                $$ language 'plpgsql';
            """)

            # Function to reset failed login attempts
            cursor.execute("""
                CREATE OR REPLACE FUNCTION reset_failed_login_attempts(user_id_param INTEGER)
                RETURNS VOID AS $$
                BEGIN
                    UPDATE users
                    SET failed_login_attempts = 0,
                        locked_until = NULL
                    WHERE id = user_id_param;
                END;
                $$ language 'plpgsql';
            """)

            # Function for audit logging
            cursor.execute("""
                CREATE OR REPLACE FUNCTION audit_action(
                    user_id_param INTEGER,
                    action_param TEXT,
                    resource_type_param TEXT,
                    resource_id_param TEXT DEFAULT NULL,
                    ip_param INET DEFAULT NULL,
                    details_param JSONB DEFAULT '{}'
                )
                RETURNS VOID AS $$
                BEGIN
                    INSERT INTO audit_log (user_id, action, resource_type, resource_id, ip_address, details)
                    VALUES (user_id_param, action_param, resource_type_param, resource_id_param, ip_param, details_param);
                END;
                $$ language 'plpgsql';
            """)

            print("Database security functions created")

    def encrypt_existing_config(self):
        """Encrypt existing sensitive configuration values"""
        print("Encrypting existing configuration...")

        # Get current config
        config_file = Path("envyro_config.json")
        if config_file.exists():
            with open(config_file, 'r') as f:
                config = json.load(f)

            # Encrypt sensitive values
            sensitive_keys = {'db_password', 'api_key', 'secret_key', 'jwt_secret'}
            for key in sensitive_keys:
                if key in config and config[key]:
                    config[key] = self.security.encrypt_data(str(config[key]))

            # Save encrypted config
            with open(config_file, 'w') as f:
                json.dump(config, f, indent=2)

            print("Configuration encrypted")
        else:
            print("No configuration file found")

    def run_migration(self):
        """Run the complete security migration"""
        print("Starting Envyro Security Migration...")
        print("=" * 50)

        conn = None
        try:
            conn = self.get_db_connection()

            # Step 1: Backup existing data
            backup_file = self.backup_existing_data(conn)
            if not backup_file:
                print("Migration aborted due to backup failure")
                return False

            # Step 2: Migrate existing tables
            self.migrate_users_table(conn)
            self.migrate_envyro_knowledge_table(conn)

            # Step 3: Create new tables
            self.create_new_tables(conn)

            # Step 4: Create database functions
            self.create_database_functions(conn)

            # Step 5: Encrypt existing config
            self.encrypt_existing_config()

            # Commit all changes
            conn.commit()

            print("=" * 50)
            print("Security Migration Completed Successfully!")
            print("Backup file created:", backup_file)
            print("\nNext steps:")
            print("1. Restart all Envyro services")
            print("2. Test authentication with existing users")
            print("3. Verify encrypted data access")

            return True

        except Exception as e:
            print(f"Migration failed: {e}")
            if conn:
                conn.rollback()
            return False

        finally:
            if conn:
                conn.close()

if __name__ == "__main__":
    migration = SecurityMigration()
    success = migration.run_migration()
    sys.exit(0 if success else 1)