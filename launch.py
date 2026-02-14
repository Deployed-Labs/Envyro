#!/usr/bin/env python3
"""
Envyro Web Launcher Script
Starts the Envyro web-based GUI application for managing services and uploading files.
"""

import sys
import os

# Add the current directory to Python path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    # Import and run directly
    import sys
    import os
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

    from envyro_launcher import SecureEnvyroWebLauncher

    # Create and run the launcher
    launcher = SecureEnvyroWebLauncher()
    launcher.run()
except ImportError as e:
    print(f"Error: {e}")
    print("Please ensure all dependencies are installed:")
    print("pip install -r requirements.txt")
    sys.exit(1)
except KeyboardInterrupt:
    print("\nEnvyro Web Launcher closed.")
    sys.exit(0)