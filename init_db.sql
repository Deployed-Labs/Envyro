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
-- SECURITY WARNING: The default Admiral account should be created manually in production.
-- 
-- For development/testing only, uncomment the following line to create an Admiral with password "admin":
-- (The hash below is bcrypt of "admin" with cost factor 12)
--
-- INSERT INTO users (username, password_hash, role) 
-- VALUES ('admin', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYqXfj1K.5W', 'admiral')
-- ON CONFLICT (username) DO NOTHING;
--
-- For production, create the Admiral manually with a strong password:
-- 1. Generate a bcrypt hash: python -c "import bcrypt; print(bcrypt.hashpw(b'YOUR_STRONG_PASSWORD', bcrypt.gensalt(12)).decode())"
-- 2. INSERT INTO users (username, password_hash, role) VALUES ('admin', 'YOUR_HASH_HERE', 'admiral');

-- Grant necessary permissions (adjust as needed for your environment)
-- GRANT ALL PRIVILEGES ON TABLE envyro_knowledge TO your_user;
-- GRANT ALL PRIVILEGES ON TABLE users TO your_user;
