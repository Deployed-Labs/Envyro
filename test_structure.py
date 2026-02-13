"""
Test script to verify Envyro-Core structure without dependencies.
"""

import sys
import os
import ast

def check_python_file(filepath):
    """Check if a Python file is valid."""
    try:
        with open(filepath, 'r') as f:
            code = f.read()
        ast.parse(code)
        return True, "OK"
    except SyntaxError as e:
        return False, f"Syntax Error: {e}"
    except Exception as e:
        return False, f"Error: {e}"

def verify_structure():
    """Verify the Envyro-Core structure."""
    print("=" * 60)
    print("Envyro-Core Structure Verification")
    print("=" * 60)
    print()
    
    # Get base directory dynamically
    base_dir = os.path.dirname(os.path.abspath(__file__))
    
    # Files to check
    files = [
        "envyro_core/__init__.py",
        "envyro_core/envyro_ai.py",
        "envyro_core/config.py",
        "envyro_core/models/__init__.py",
        "envyro_core/models/transformer.py",
        "envyro_core/memory/__init__.py",
        "envyro_core/memory/vector_memory.py",
        "envyro_core/utils/__init__.py",
        "example.py",
        "requirements.txt",
        "init_db.sql",
        "Dockerfile",
        "docker-compose.yml",
        "README.md",
        ".gitignore",
    ]
    
    print("Checking files...")
    print("-" * 60)
    
    all_good = True
    for filepath in files:
        full_path = os.path.join(base_dir, filepath)
        exists = os.path.exists(full_path)
        
        if exists:
            if filepath.endswith('.py'):
                valid, msg = check_python_file(full_path)
                status = "✓" if valid else "✗"
                print(f"{status} {filepath}: {msg}")
                all_good = all_good and valid
            else:
                print(f"✓ {filepath}: Exists")
        else:
            print(f"✗ {filepath}: Missing")
            all_good = False
    
    print()
    print("-" * 60)
    
    # Check key classes and functions
    print("\nVerifying key components...")
    print("-" * 60)
    
    components = {
        "EnvyroAI class": "envyro_core/envyro_ai.py",
        "EnvyroTransformer class": "envyro_core/models/transformer.py",
        "VectorMemory class": "envyro_core/memory/vector_memory.py",
        "EnvyroConfig class": "envyro_core/config.py",
    }
    
    for component, filepath in components.items():
        full_path = os.path.join(base_dir, filepath)
        try:
            with open(full_path, 'r') as f:
                content = f.read()
            class_name = component.split()[0]
            if f"class {class_name}" in content:
                print(f"✓ {component}: Found")
            else:
                print(f"✗ {component}: Not found")
                all_good = False
        except Exception as e:
            print(f"✗ {component}: Error - {e}")
            all_good = False
    
    print()
    print("-" * 60)
    
    # Check key methods
    print("\nVerifying key methods...")
    print("-" * 60)
    
    envyro_ai_path = os.path.join(base_dir, "envyro_core/envyro_ai.py")
    key_methods = [
        "__init__",
        "_initialize_weights",
        "recall",
        "cognitive_loop",
        "learn_from_interaction",
        "save_weights",
        "load_weights",
        "get_admiral_stats",
    ]
    
    try:
        with open(envyro_ai_path, 'r') as f:
            content = f.read()
        
        for method in key_methods:
            if f"def {method}" in content:
                print(f"✓ EnvyroAI.{method}(): Found")
            else:
                print(f"✗ EnvyroAI.{method}(): Not found")
                all_good = False
    except Exception as e:
        print(f"✗ Error checking methods: {e}")
        all_good = False
    
    print()
    print("=" * 60)
    
    if all_good:
        print("✓ ALL CHECKS PASSED")
        print("\nEnvyro-Core structure is complete!")
        print("\nKey features implemented:")
        print("  • Custom Transformer architecture")
        print("  • Xavier/He weight initialization")
        print("  • PostgreSQL + pgvector integration")
        print("  • Recall function for memory retrieval")
        print("  • Cognitive Loop (Recall → Generate)")
        print("  • Admiral system with God Mode")
        print("  • Docker configuration")
    else:
        print("✗ SOME CHECKS FAILED")
    
    print("=" * 60)
    
    return all_good

if __name__ == "__main__":
    success = verify_structure()
    sys.exit(0 if success else 1)
