-- Envyro Database Initialization Script
-- PostgreSQL with pgvector extension for AI Long-Term Memory

-- Enable the vector extension for AI memory
CREATE EXTENSION IF NOT EXISTS vector;

-- Create the Envyro Memory Table
CREATE TABLE IF NOT EXISTS envyro_knowledge (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL,          -- The actual text/fact
    embedding VECTOR(1536),         -- The mathematical "thought" vector
    created_by TEXT DEFAULT 'system',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Create the User Table with the Admiral
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT DEFAULT 'user'        -- 'admiral', 'user', or 'sprout'
);

-- Create index for faster user lookups
CREATE INDEX IF NOT EXISTS users_username_idx ON users(username);
CREATE INDEX IF NOT EXISTS users_role_idx ON users(role);

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
-- GRANT ALL PRIVILEGES ON TABLE envyro_knowledge TO your_user;
-- GRANT ALL PRIVILEGES ON TABLE users TO your_user;
