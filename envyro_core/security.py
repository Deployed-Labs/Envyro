"""
Envyro Security & Encryption System
Provides comprehensive encryption, access control, and security utilities.
"""

import os
import json
import base64
import secrets
import hashlib
from typing import Dict, Any, Optional, List
from datetime import datetime, timedelta
from functools import wraps
import jwt as pyjwt
from cryptography.fernet import Fernet
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives.asymmetric import rsa, padding
from cryptography.hazmat.primitives import serialization
import logging

logger = logging.getLogger(__name__)


class EnvyroSecurity:
    """
    Core security and encryption system for Envyro.
    Handles encryption, decryption, access control, and key management.
    """

    def __init__(self, master_key: Optional[str] = None):
        """
        Initialize the security system.

        Args:
            master_key: Master encryption key (generated if not provided)
        """
        self.master_key = master_key or self._generate_master_key()
        self.fernet = Fernet(self.master_key)

        # Generate RSA key pair for asymmetric encryption
        self.private_key = rsa.generate_private_key(
            public_exponent=65537,
            key_size=2048
        )
        self.public_key = self.private_key.public_key()

        # JWT secret for token signing
        self.jwt_secret = secrets.token_hex(32)

        # Access control roles
        self.roles = {
            'admin': ['read', 'write', 'delete', 'admin'],
            'user': ['read', 'write'],
            'guest': ['read']
        }

        logger.info("Envyro Security system initialized")

    def _generate_master_key(self) -> bytes:
        """Generate a new master encryption key."""
        # Use PBKDF2 to derive a key from a random salt
        salt = secrets.token_bytes(16)
        kdf = PBKDF2HMAC(
            algorithm=hashes.SHA256(),
            length=32,
            salt=salt,
            iterations=100000,
        )
        key = base64.urlsafe_b64encode(kdf.derive(secrets.token_bytes(32)))
        return key

    def encrypt_data(self, data: str) -> str:
        """
        Encrypt string data using symmetric encryption.

        Args:
            data: Data to encrypt

        Returns:
            Base64 encoded encrypted data
        """
        if not isinstance(data, str):
            data = str(data)
        encrypted = self.fernet.encrypt(data.encode())
        return base64.b64encode(encrypted).decode()

    def decrypt_data(self, encrypted_data: str) -> str:
        """
        Decrypt data using symmetric encryption.

        Args:
            encrypted_data: Base64 encoded encrypted data

        Returns:
            Decrypted string data
        """
        try:
            encrypted = base64.b64decode(encrypted_data)
            decrypted = self.fernet.decrypt(encrypted)
            return decrypted.decode()
        except Exception as e:
            logger.error(f"Decryption failed: {e}")
            raise ValueError("Invalid encrypted data")

    def encrypt_file(self, file_path: str, output_path: Optional[str] = None) -> str:
        """
        Encrypt a file.

        Args:
            file_path: Path to file to encrypt
            output_path: Path for encrypted file (optional)

        Returns:
            Path to encrypted file
        """
        if not output_path:
            output_path = file_path + '.encrypted'

        with open(file_path, 'rb') as f:
            data = f.read()

        encrypted = self.fernet.encrypt(data)

        with open(output_path, 'wb') as f:
            f.write(encrypted)

        return output_path

    def decrypt_file(self, encrypted_file_path: str, output_path: Optional[str] = None) -> str:
        """
        Decrypt a file.

        Args:
            encrypted_file_path: Path to encrypted file
            output_path: Path for decrypted file (optional)

        Returns:
            Path to decrypted file
        """
        if not output_path:
            output_path = encrypted_file_path.replace('.encrypted', '')

        with open(encrypted_file_path, 'rb') as f:
            encrypted_data = f.read()

        decrypted = self.fernet.decrypt(encrypted_data)

        with open(output_path, 'wb') as f:
            f.write(decrypted)

        return output_path

    def hash_password(self, password: str) -> str:
        """
        Hash a password using bcrypt.

        Args:
            password: Plain text password

        Returns:
            Hashed password
        """
        import bcrypt
        salt = bcrypt.gensalt(rounds=12)
        return bcrypt.hashpw(password.encode(), salt).decode()

    def verify_password(self, password: str, hashed: str) -> bool:
        """
        Verify a password against its hash.

        Args:
            password: Plain text password
            hashed: Hashed password

        Returns:
            True if password matches
        """
        import bcrypt
        return bcrypt.checkpw(password.encode(), hashed.encode())

    def generate_token(self, user_id: str, role: str = 'user', expires_in: int = 3600) -> str:
        """
        Generate a JWT token for user authentication.

        Args:
            user_id: User identifier
            role: User role
            expires_in: Token expiration time in seconds

        Returns:
            JWT token
        """
        payload = {
            'user_id': user_id,
            'role': role,
            'exp': datetime.utcnow() + timedelta(seconds=expires_in),
            'iat': datetime.utcnow()
        }
        return pyjwt.encode(payload, self.jwt_secret, algorithm='HS256')

    def verify_token(self, token: str) -> Optional[Dict[str, Any]]:
        """
        Verify and decode a JWT token.

        Args:
            token: JWT token

        Returns:
            Decoded payload or None if invalid
        """
        try:
            payload = pyjwt.decode(token, self.jwt_secret, algorithms=['HS256'])
            return payload
        except pyjwt.ExpiredSignatureError:
            logger.warning("Token has expired")
            return None
        except pyjwt.InvalidTokenError:
            logger.warning("Invalid token")
            return None

    def check_permission(self, user_role: str, required_permission: str) -> bool:
        """
        Check if a user role has a required permission.

        Args:
            user_role: User's role
            required_permission: Required permission

        Returns:
            True if user has permission
        """
        if user_role not in self.roles:
            return False
        return required_permission in self.roles[user_role]

    def encrypt_config_value(self, key: str, value: str) -> str:
        """
        Encrypt a configuration value.

        Args:
            key: Configuration key
            value: Configuration value

        Returns:
            Encrypted value with metadata
        """
        data = {
            'key': key,
            'value': value,
            'timestamp': datetime.utcnow().isoformat()
        }
        json_data = json.dumps(data)
        return self.encrypt_data(json_data)

    def decrypt_config_value(self, encrypted_value: str) -> Dict[str, Any]:
        """
        Decrypt a configuration value.

        Args:
            encrypted_value: Encrypted configuration value

        Returns:
            Decrypted configuration data
        """
        json_data = self.decrypt_data(encrypted_value)
        return json.loads(json_data)

    def generate_secure_filename(self, original_filename: str) -> str:
        """
        Generate a secure filename with random component.

        Args:
            original_filename: Original filename

        Returns:
            Secure filename
        """
        random_part = secrets.token_hex(8)
        name, ext = os.path.splitext(original_filename)
        return f"{name}_{random_part}{ext}"

    def sanitize_filename(self, filename: str) -> str:
        """
        Sanitize filename to prevent path traversal attacks.

        Args:
            filename: Filename to sanitize

        Returns:
            Sanitized filename
        """
        # Remove path separators and dangerous characters
        dangerous_chars = ['/', '\\', '..', '<', '>', ':', '*', '?', '"', '|']
        for char in dangerous_chars:
            filename = filename.replace(char, '_')
        return filename.strip()

    def audit_log(self, action: str, user_id: str, resource: str, details: Optional[Dict] = None):
        """
        Log security-related actions for audit purposes.

        Args:
            action: Action performed
            user_id: User who performed the action
            resource: Resource affected
            details: Additional details
        """
        log_entry = {
            'timestamp': datetime.utcnow().isoformat(),
            'action': action,
            'user_id': user_id,
            'resource': resource,
            'details': details or {}
        }
        logger.info(f"AUDIT: {json.dumps(log_entry)}")


class AccessControl:
    """
    Role-based access control system.
    """

    def __init__(self, security: EnvyroSecurity):
        self.security = security

    def require_permission(self, permission: str):
        """
        Decorator to require specific permission for a function.

        Args:
            permission: Required permission

        Returns:
            Decorated function
        """
        def decorator(func):
            @wraps(func)
            def wrapper(*args, **kwargs):
                # Extract token from request context (Flask)
                try:
                    from flask import request
                    token = request.headers.get('Authorization', '').replace('Bearer ', '')
                    if not token:
                        return {'error': 'No authentication token provided'}, 401

                    payload = self.security.verify_token(token)
                    if not payload:
                        return {'error': 'Invalid or expired token'}, 401

                    user_role = payload.get('role', 'guest')
                    if not self.security.check_permission(user_role, permission):
                        self.security.audit_log(
                            'access_denied',
                            payload.get('user_id', 'unknown'),
                            func.__name__,
                            {'permission': permission}
                        )
                        return {'error': 'Insufficient permissions'}, 403

                    # Add user info to request context
                    request.user = payload
                    return func(*args, **kwargs)

                except ImportError:
                    # Not in Flask context, allow for testing
                    return func(*args, **kwargs)

            return wrapper
        return decorator

    def require_admin(self):
        """Decorator to require admin permissions."""
        return self.require_permission('admin')

    def require_auth(self):
        """Decorator to require authentication (any role)."""
        def decorator(func):
            @wraps(func)
            def wrapper(*args, **kwargs):
                try:
                    from flask import request
                    token = request.headers.get('Authorization', '').replace('Bearer ', '')
                    if not token:
                        return {'error': 'No authentication token provided'}, 401

                    payload = self.security.verify_token(token)
                    if not payload:
                        return {'error': 'Invalid or expired token'}, 401

                    request.user = payload
                    return func(*args, **kwargs)

                except ImportError:
                    return func(*args, **kwargs)

            return wrapper
        return decorator


# Global security instance
_security_instance = None

def get_security_instance() -> EnvyroSecurity:
    """Get the global security instance."""
    global _security_instance
    if _security_instance is None:
        _security_instance = EnvyroSecurity()
    return _security_instance

def get_access_control() -> AccessControl:
    """Get the global access control instance."""
    return AccessControl(get_security_instance())