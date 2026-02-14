"""
Secure Configuration Manager
Handles encrypted configuration storage and retrieval.
"""

import os
import json
from pathlib import Path
from typing import Dict, Any, Optional, List
from dotenv import load_dotenv, set_key, unset_key
from security import EnvyroSecurity
import logging

logger = logging.getLogger(__name__)


class SecureConfig:
    """
    Secure configuration manager with encryption for sensitive values.
    """

    # Keys that should be encrypted
    SENSITIVE_KEYS = {
        'POSTGRES_PASSWORD',
        'ENVYRO_DB_PASSWORD',
        'JWT_SECRET',
        'MASTER_KEY',
        'API_KEY',
        'SECRET_KEY',
        'ENCRYPTION_KEY'
    }

    def __init__(self, env_file: str = '.env'):
        """
        Initialize secure configuration.

        Args:
            env_file: Path to environment file
        """
        self.env_file = Path(env_file)
        self.security = EnvyroSecurity()
        self._config_cache = {}
        self._load_config()

    def _load_config(self):
        """Load configuration from environment file."""
        if self.env_file.exists():
            load_dotenv(self.env_file)
            self._config_cache = dict(os.environ)
        else:
            self._config_cache = {}

    def get(self, key: str, default: Any = None) -> Any:
        """
        Get a configuration value, decrypting if necessary.

        Args:
            key: Configuration key
            default: Default value if key not found

        Returns:
            Configuration value
        """
        value = self._config_cache.get(key, os.getenv(key, default))

        if value and key in self.SENSITIVE_KEYS:
            try:
                # Try to decrypt if it's encrypted
                decrypted_data = self.security.decrypt_config_value(value)
                return decrypted_data['value']
            except (ValueError, json.JSONDecodeError):
                # Not encrypted, return as-is
                pass

        return value

    def set(self, key: str, value: Any, encrypt: bool = None):
        """
        Set a configuration value, encrypting if sensitive.

        Args:
            key: Configuration key
            value: Configuration value
            encrypt: Force encryption (auto-detected if None)
        """
        if encrypt is None:
            encrypt = key in self.SENSITIVE_KEYS

        if encrypt and value:
            encrypted_value = self.security.encrypt_config_value(key, str(value))
            self._config_cache[key] = encrypted_value
            set_key(self.env_file, key, encrypted_value)
        else:
            self._config_cache[key] = str(value)
            set_key(self.env_file, key, str(value))

        # Update environment
        os.environ[key] = str(value)

    def delete(self, key: str):
        """
        Delete a configuration key.

        Args:
            key: Configuration key to delete
        """
        if key in self._config_cache:
            del self._config_cache[key]

        if self.env_file.exists():
            unset_key(self.env_file, key)

        if key in os.environ:
            del os.environ[key]

    def get_all(self) -> Dict[str, Any]:
        """
        Get all configuration values (with sensitive values masked).

        Returns:
            Dictionary of configuration values
        """
        result = {}
        for key, value in self._config_cache.items():
            if key in self.SENSITIVE_KEYS and value:
                result[key] = "***ENCRYPTED***"
            else:
                result[key] = value
        return result

    def get_all_decrypted(self) -> Dict[str, Any]:
        """
        Get all configuration values with sensitive values decrypted.

        Returns:
            Dictionary of all configuration values
        """
        result = {}
        for key in self._config_cache.keys():
            result[key] = self.get(key)
        return result

    def save_to_file(self, filepath: str):
        """
        Save current configuration to a file.

        Args:
            filepath: Path to save configuration
        """
        config = self.get_all_decrypted()
        with open(filepath, 'w') as f:
            json.dump(config, f, indent=2)

    def load_from_file(self, filepath: str):
        """
        Load configuration from a file.

        Args:
            filepath: Path to load configuration from
        """
        with open(filepath, 'r') as f:
            config = json.load(f)

        for key, value in config.items():
            self.set(key, value)

    def generate_secure_config(self):
        """Generate secure default configuration values."""
        # Generate secure passwords and keys
        db_password = self.security.hash_password(os.urandom(16).hex())[:16]

        config_updates = {
            'POSTGRES_PASSWORD': db_password,
            'ENVYRO_DB_PASSWORD': db_password,
            'JWT_SECRET': self.security.jwt_secret,
            'MASTER_KEY': self.security.master_key.decode(),
            'SECRET_KEY': os.urandom(32).hex()
        }

        for key, value in config_updates.items():
            if not self.get(key):
                self.set(key, value, encrypt=True)

        logger.info("Secure configuration generated")

    def validate_config(self) -> List[str]:
        """
        Validate current configuration for security issues.

        Returns:
            List of validation warnings/errors
        """
        warnings = []

        # Check for weak passwords
        password_keys = ['POSTGRES_PASSWORD', 'ENVYRO_DB_PASSWORD']
        for key in password_keys:
            password = self.get(key)
            if password and len(password) < 12:
                warnings.append(f"Password for {key} is too weak (minimum 12 characters)")

        # Check for default values
        if self.get('POSTGRES_PASSWORD') == 'envyro123':
            warnings.append("Using default PostgreSQL password - change immediately!")

        # Check for missing encryption
        for key in self.SENSITIVE_KEYS:
            value = os.getenv(key)
            if value and not self._is_encrypted(value):
                warnings.append(f"Sensitive key {key} is not encrypted")

        return warnings

    def _is_encrypted(self, value: str) -> bool:
        """
        Check if a value appears to be encrypted.

        Args:
            value: Value to check

        Returns:
            True if value appears encrypted
        """
        try:
            # Try to decrypt - if it works and contains our metadata, it's encrypted
            decrypted = self.security.decrypt_config_value(value)
            return 'key' in decrypted and 'value' in decrypted
        except:
            return False


# Global config instance
_config_instance = None

def get_secure_config() -> SecureConfig:
    """Get the global secure configuration instance."""
    global _config_instance
    if _config_instance is None:
        _config_instance = SecureConfig()
    return _config_instance