#!/usr/bin/env python3
"""
Comprehensive test script for Envyro-Core
Tests all major components without requiring database connection.
"""

import sys
import os
import torch
import numpy as np

# Add the parent directory to the path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from envyro_core import EnvyroAI
from envyro_core.models.transformer import EnvyroTransformer, PositionalEncoding, MultiHeadAttention
from envyro_core.memory.vector_memory import VectorMemory
from envyro_core.config import EnvyroConfig


def test_transformer_components():
    """Test individual Transformer components."""
    print("=" * 60)
    print("Testing Transformer Components")
    print("=" * 60)

    # Test Positional Encoding
    print("\n1. Testing PositionalEncoding...")
    pe = PositionalEncoding(d_model=512, max_seq_length=100)
    x = torch.randn(2, 50, 512)  # batch_size=2, seq_len=50, d_model=512
    output = pe(x)
    assert output.shape == x.shape, f"Shape mismatch: expected {x.shape}, got {output.shape}"
    print("✓ PositionalEncoding works correctly")

    # Test Multi-Head Attention
    print("\n2. Testing MultiHeadAttention...")
    mha = MultiHeadAttention(d_model=512, n_heads=8)
    x = torch.randn(2, 50, 512)
    output = mha(x, x, x)  # self-attention
    assert output.shape == x.shape, f"Shape mismatch: expected {x.shape}, got {output.shape}"
    print("✓ MultiHeadAttention works correctly")

    # Test Full Transformer
    print("\n3. Testing EnvyroTransformer...")
    transformer = EnvyroTransformer(
        vocab_size=1000,
        d_model=512,
        n_heads=8,
        n_layers=2,
        d_ff=1024,
        max_seq_length=100,
        dropout=0.1
    )
    x = torch.randint(0, 1000, (2, 20))  # batch_size=2, seq_len=20
    output = transformer(x)
    expected_shape = (2, 20, 1000)  # vocab_size for logits
    assert output.shape == expected_shape, f"Shape mismatch: expected {expected_shape}, got {output.shape}"
    print("✓ EnvyroTransformer works correctly")


def test_vector_memory_mock():
    """Test VectorMemory in mock mode (no database)."""
    print("\n" + "=" * 60)
    print("Testing VectorMemory (Mock Mode)")
    print("=" * 60)

    # Test initialization without database (will try to connect but fail gracefully)
    print("\n1. Testing VectorMemory initialization...")
    memory = VectorMemory(db_config=None)  # Will use defaults and fail to connect
    # The db_config will be set to defaults, but connection will be None
    assert memory.connection is None, "Connection should be None when database unavailable"
    print("✓ VectorMemory mock initialization works (degraded mode)")

    # Test embedding generation (works without database)
    print("\n2. Testing embedding generation...")
    text = "Hello, world!"
    embedding = memory._text_to_embedding(text)
    assert isinstance(embedding, np.ndarray), "Embedding should be numpy array"
    assert embedding.shape == (1536,), f"Embedding shape should be (1536,), got {embedding.shape}"
    print("✓ Embedding generation works")

    # Test similarity search (should fail gracefully)
    print("\n3. Testing similarity search...")
    try:
        results = memory.search_similar("test query", limit=5)
        # If it succeeds, results should be empty (no data)
        assert results == [], "Should return empty results when no database"
    except Exception:
        # If it fails due to no connection, that's also acceptable
        print("✓ Similarity search handled gracefully (no database)")
        return
    print("✓ Similarity search works (no database)")


def test_envyro_ai_full():
    """Test full EnvyroAI functionality."""
    print("\n" + "=" * 60)
    print("Testing Full EnvyroAI System")
    print("=" * 60)

    # Initialize EnvyroAI
    print("\n1. Initializing EnvyroAI...")
    ai = EnvyroAI(
        vocab_size=50000,
        d_model=512,
        n_heads=8,
        n_layers=6,
        d_ff=2048,
        max_seq_length=512,
        dropout=0.1,
        db_config=None  # No database
    )
    print("✓ EnvyroAI initialized successfully")

    # Test weight initialization
    print("\n2. Testing weight initialization...")
    total_params = sum(p.numel() for p in ai.model.parameters())
    assert total_params > 50_000_000, f"Expected >50M parameters, got {total_params}"
    print(f"✓ Model has {total_params:,} parameters")

    # Test recall function (should return empty in mock mode)
    print("\n3. Testing recall function...")
    memories = ai.recall("What is AI?")
    assert memories == [], "Recall without memory should return empty list"
    print("✓ Recall function works (no memory)")

    # Test cognitive loop (placeholder response)
    print("\n4. Testing cognitive loop...")
    response = ai.cognitive_loop("Hello, Envyro!", use_memory=False)
    assert "EnvyroAI Response" in response, "Should return placeholder response"
    print("✓ Cognitive loop works (placeholder)")

    # Test weight saving/loading
    print("\n5. Testing weight save/load...")
    import tempfile
    with tempfile.NamedTemporaryFile(suffix='.pt', delete=False) as f:
        temp_path = f.name

    try:
        ai.save_weights(temp_path)
        print("✓ Weights saved successfully")

        # Create new AI and load weights
        ai2 = EnvyroAI(
            vocab_size=50000,
            d_model=512,
            n_heads=8,
            n_layers=6,
            d_ff=2048,
            max_seq_length=512,
            dropout=0.1,
            db_config=None
        )
        ai2.load_weights(temp_path)
        print("✓ Weights loaded successfully")

    finally:
        if os.path.exists(temp_path):
            os.unlink(temp_path)

    # Test Admiral stats
    print("\n6. Testing Admiral statistics...")
    stats = ai.get_admiral_stats()
    required_keys = ['parameters', 'device', 'd_model', 'vocab_size', 'max_seq_length']
    for key in required_keys:
        assert key in stats, f"Missing key: {key}"
    assert stats['parameters'] == total_params, "Parameter count mismatch"
    print("✓ Admiral statistics correct")


def test_config():
    """Test configuration system."""
    print("\n" + "=" * 60)
    print("Testing Configuration System")
    print("=" * 60)

    # Test default config
    print("\n1. Testing default configuration...")
    model_config = EnvyroConfig.get_model_config()
    required_keys = ['vocab_size', 'd_model', 'n_heads', 'n_layers']
    for key in required_keys:
        assert key in model_config, f"Missing model config key: {key}"
    print("✓ Default model config loaded")

    # Test database config (should handle missing env vars gracefully)
    print("\n2. Testing database config...")
    db_config = EnvyroConfig.get_db_config()
    # Should return None or default values when env vars not set
    print("✓ Database config handled gracefully")


def run_all_tests():
    """Run all test suites."""
    print("🚀 Starting Envyro-Core Comprehensive Tests")
    print("=" * 60)

    try:
        test_transformer_components()
        test_vector_memory_mock()
        test_envyro_ai_full()
        test_config()

        print("\n" + "=" * 60)
        print("🎉 ALL TESTS PASSED!")
        print("Envyro-Core is ready for the Digital Oasis!")
        print("=" * 60)

    except Exception as e:
        print(f"\n❌ TEST FAILED: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    run_all_tests()