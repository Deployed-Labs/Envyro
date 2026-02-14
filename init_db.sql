-- Envyro Database Initialization Script
-- PostgreSQL with pgvector extension for AI Long-Term Memory
-- Enhanced with security and encryption support

-- Enable the vector extension for AI memory
CREATE EXTENSION IF NOT EXISTS vector;

-- Create the Envyro Memory Table with encryption support
CREATE TABLE IF NOT EXISTS envyro_knowledge (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL,          -- The actual text/fact (encrypted)
    embedding VECTOR(1536),         -- The mathematical "thought" vector
    created_by TEXT DEFAULT 'system',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    access_level TEXT DEFAULT 'public', -- 'public', 'user', 'admin'
    metadata JSONB DEFAULT '{}'     -- Encrypted metadata
);

-- Create the User Table with enhanced security
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT DEFAULT 'user' CHECK (role IN ('admiral', 'user', 'guest')),
    email TEXT,                     -- Encrypted
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    last_login TIMESTAMP WITH TIME ZONE,
    is_active BOOLEAN DEFAULT TRUE,
    failed_login_attempts INTEGER DEFAULT 0,
    locked_until TIMESTAMP WITH TIME ZONE,
    preferences JSONB DEFAULT '{}'  -- Encrypted user preferences
);

-- Create the Sessions Table for secure session management
CREATE TABLE IF NOT EXISTS user_sessions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    session_token TEXT UNIQUE NOT NULL,  -- Encrypted
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    is_active BOOLEAN DEFAULT TRUE
);

-- Create the File Storage Table for encrypted file management
CREATE TABLE IF NOT EXISTS file_storage (
    id SERIAL PRIMARY KEY,
    filename TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    file_path TEXT NOT NULL,        -- Encrypted path to encrypted file
    file_size BIGINT,
    mime_type TEXT,
    uploaded_by INTEGER REFERENCES users(id),
    uploaded_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    access_level TEXT DEFAULT 'private' CHECK (access_level IN ('public', 'user', 'admin')),
    checksum TEXT,                  -- SHA256 hash for integrity
    metadata JSONB DEFAULT '{}'     -- Encrypted metadata
);

-- Create the Audit Log Table for security tracking
CREATE TABLE IF NOT EXISTS audit_log (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER REFERENCES users(id),
    action TEXT NOT NULL,           -- 'login', 'logout', 'upload', 'download', etc.
    resource_type TEXT NOT NULL,    -- 'file', 'config', 'user', etc.
    resource_id TEXT,               -- ID of the affected resource
    ip_address INET,
    user_agent TEXT,
    details JSONB DEFAULT '{}',     -- Encrypted details
    success BOOLEAN DEFAULT TRUE
);

-- Create indexes for performance and security
CREATE INDEX IF NOT EXISTS users_username_idx ON users(username);
CREATE INDEX IF NOT EXISTS users_role_idx ON users(role);
CREATE INDEX IF NOT EXISTS users_active_idx ON users(is_active);
CREATE INDEX IF NOT EXISTS user_sessions_token_idx ON user_sessions(session_token);
CREATE INDEX IF NOT EXISTS user_sessions_expires_idx ON user_sessions(expires_at);
CREATE INDEX IF NOT EXISTS file_storage_uploaded_by_idx ON file_storage(uploaded_by);
CREATE INDEX IF NOT EXISTS file_storage_access_level_idx ON file_storage(access_level);
CREATE INDEX IF NOT EXISTS envyro_knowledge_access_level_idx ON envyro_knowledge(access_level);
CREATE INDEX IF NOT EXISTS audit_log_timestamp_idx ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS audit_log_user_idx ON audit_log(user_id);

-- Create function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create trigger for automatic timestamp updates
CREATE TRIGGER update_envyro_knowledge_updated_at
    BEFORE UPDATE ON envyro_knowledge
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Create function for failed login attempt tracking
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

-- Create function to reset failed login attempts
CREATE OR REPLACE FUNCTION reset_failed_login_attempts(user_id_param INTEGER)
RETURNS VOID AS $$
BEGIN
    UPDATE users
    SET failed_login_attempts = 0,
        locked_until = NULL
    WHERE id = user_id_param;
END;
$$ language 'plpgsql';

-- Create function for audit logging
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

-- Admiral Account Setup
-- =====================
-- SECURITY: No default Admiral account is created for security reasons.
--
-- Use the setup_admiral.py script to create an Admiral account:
--   python setup_admiral.py
--
-- For manual creation in development/testing only:
--   1. Generate a bcrypt hash:
--      python -c "import bcrypt; print(bcrypt.hashpw(b'YOUR_PASSWORD', bcrypt.gensalt(12)).decode())"
--   2. INSERT INTO users (username, password_hash, role)
--      VALUES ('your_username', 'YOUR_BCRYPT_HASH', 'admiral');
--
-- NEVER use weak passwords like 'admin', 'password', etc.

-- Grant necessary permissions (adjust as needed for your environment)
-- Note: In production, use specific grants instead of ALL PRIVILEGES
-- GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO envyro_user;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO envyro_user;
