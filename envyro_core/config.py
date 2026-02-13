"""
Configuration for Envyro-Core
"""

import os
from typing import Dict


class EnvyroConfig:
    """
    Configuration class for EnvyroAI.
    Loads settings from environment variables or uses defaults.
    """
    
    # Model Configuration
    VOCAB_SIZE = int(os.getenv('ENVYRO_VOCAB_SIZE', 50000))
    D_MODEL = int(os.getenv('ENVYRO_D_MODEL', 512))
    N_HEADS = int(os.getenv('ENVYRO_N_HEADS', 8))
    N_LAYERS = int(os.getenv('ENVYRO_N_LAYERS', 6))
    D_FF = int(os.getenv('ENVYRO_D_FF', 2048))
    MAX_SEQ_LENGTH = int(os.getenv('ENVYRO_MAX_SEQ_LENGTH', 512))
    DROPOUT = float(os.getenv('ENVYRO_DROPOUT', 0.1))
    
    # Database Configuration
    DB_HOST = os.getenv('ENVYRO_DB_HOST', 'localhost')
    DB_PORT = int(os.getenv('ENVYRO_DB_PORT', 5432))
    DB_NAME = os.getenv('ENVYRO_DB_NAME', 'envyro')
    DB_USER = os.getenv('ENVYRO_DB_USER', 'postgres')
    DB_PASSWORD = os.getenv('ENVYRO_DB_PASSWORD', 'postgres')
    
    @classmethod
    def get_db_config(cls) -> Dict:
        """Get database configuration as dictionary."""
        return {
            'host': cls.DB_HOST,
            'port': cls.DB_PORT,
            'database': cls.DB_NAME,
            'user': cls.DB_USER,
            'password': cls.DB_PASSWORD
        }
    
    @classmethod
    def get_model_config(cls) -> Dict:
        """Get model configuration as dictionary."""
        return {
            'vocab_size': cls.VOCAB_SIZE,
            'd_model': cls.D_MODEL,
            'n_heads': cls.N_HEADS,
            'n_layers': cls.N_LAYERS,
            'd_ff': cls.D_FF,
            'max_seq_length': cls.MAX_SEQ_LENGTH,
            'dropout': cls.DROPOUT
        }
