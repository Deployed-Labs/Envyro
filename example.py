"""
Example: Basic usage of Envyro-Core
Demonstrates initialization, recall, and cognitive loop.
"""

import sys
import os

# Add the parent directory to the path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from envyro_core import EnvyroAI
from envyro_core.config import EnvyroConfig


def main():
    """
    Example usage of EnvyroAI.
    """
    print("=" * 60)
    print("Envyro-Core Example: The Digital Oasis AI")
    print("=" * 60)
    print()
    
    # Initialize EnvyroAI
    print("Initializing EnvyroAI...")
    print("-" * 60)
    
    # Option 1: Initialize without database (for testing)
    ai = EnvyroAI(
        vocab_size=50000,
        d_model=512,
        n_heads=8,
        n_layers=6,
        d_ff=2048,
        max_seq_length=512,
        dropout=0.1,
        db_config=None  # Set to None to run without database
    )
    
    # Option 2: Initialize with database connection
    # Uncomment the following lines to use PostgreSQL + pgvector
    # db_config = EnvyroConfig.get_db_config()
    # ai = EnvyroAI(
    #     **EnvyroConfig.get_model_config(),
    #     db_config=db_config
    # )
    
    print()
    print("EnvyroAI initialized successfully!")
    print()
    
    # Get Admiral stats
    print("Admiral Statistics (God Mode):")
    print("-" * 60)
    stats = ai.get_admiral_stats()
    for key, value in stats.items():
        print(f"  {key}: {value}")
    print()
    
    # Test Recall function (without database)
    print("Testing Recall Function:")
    print("-" * 60)
    query = "What is the nature of consciousness?"
    print(f"Query: {query}")
    memories = ai.recall(query, top_k=3)
    
    if memories:
        print(f"Retrieved {len(memories)} memories:")
        for i, mem in enumerate(memories, 1):
            print(f"  {i}. {mem['content'][:50]}... (similarity: {mem['similarity']:.2f})")
    else:
        print("  No memories found (database not connected)")
    print()
    
    # Test Cognitive Loop
    print("Testing Cognitive Loop:")
    print("-" * 60)
    user_input = "Hello, Envyro! Tell me about the Digital Oasis."
    print(f"User: {user_input}")
    response = ai.cognitive_loop(user_input, use_memory=False)
    print(f"EnvyroAI: {response}")
    print()
    
    # Save weights example
    print("Saving Neural Weights:")
    print("-" * 60)
    weights_path = "/tmp/envyro_weights.pt"
    ai.save_weights(weights_path)
    print(f"Weights saved to: {weights_path}")
    print()
    
    # Example with database (if configured)
    if ai.memory is not None:
        print("Testing Memory Storage:")
        print("-" * 60)
        
        # Store some knowledge
        ai.learn_from_interaction(
            query="What is Envyro?",
            response="Envyro is a self-learning AI ecosystem in a Digital Oasis club environment.",
            user_role="admiral"
        )
        print("Stored interaction in Long-Term Memory")
        print()
    
    print("=" * 60)
    print("Example complete! The Digital Oasis awaits...")
    print("=" * 60)


if __name__ == "__main__":
    main()
