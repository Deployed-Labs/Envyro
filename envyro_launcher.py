#!/usr/bin/env python3
"""
Envyro Web Launcher - Secure Web-based GUI for Managing Envyro Services
Provides authenticated web interface to start Envyro, upload files, and manage containers.
"""

import os
import sys
import subprocess
import threading
import time
import json
from pathlib import Path
from flask import Flask, render_template_string, request, jsonify, send_from_directory, session, redirect, url_for, flash
from flask_cors import CORS
import webbrowser
import logging
from functools import wraps
from datetime import datetime, timedelta

# Add the envyro_core directory to the path
sys.path.insert(0, str(Path(__file__).parent / 'envyro_core'))

from security import EnvyroSecurity
from secure_config import SecureConfig

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class SecureEnvyroWebLauncher:
    """
    Secure web-based application for managing Envyro services and file uploads.
    Includes authentication, encryption, and access control.
    """

    def __init__(self):
        self.app = Flask(__name__)
        CORS(self.app)

        # Security configuration
        self.app.config['SECRET_KEY'] = os.getenv('FLASK_SECRET_KEY', 'dev-secret-key-change-in-production')
        self.app.config['SESSION_TYPE'] = 'filesystem'
        self.app.config['PERMANENT_SESSION_LIFETIME'] = timedelta(hours=24)

        # Initialize security modules
        self.security = EnvyroSecurity()
        self.secure_config = SecureConfig()

        # Project root directory
        self.project_root = Path(__file__).parent.absolute()

        # Service states
        self.services = {
            'postgres': {'status': 'stopped', 'container': 'envyro-postgres'},
            'envyro-core': {'status': 'stopped', 'container': 'envyro-core'}
        }

        # Uploaded files
        self.uploaded_files = []

        # Environment configuration
        self.env_config = self.load_env_config()

        # Console logs
        self.console_logs = []

        self.setup_routes()
        self.update_service_status()

    def login_required(self, f):
        """Decorator to require authentication for routes."""
        @wraps(f)
        def decorated_function(*args, **kwargs):
            if 'user_id' not in session:
                return redirect('/login')
            return f(*args, **kwargs)
        return decorated_function

    def admin_required(self, f):
        """Decorator to require admin/admiral role for routes."""
        @wraps(f)
        def decorated_function(*args, **kwargs):
            if 'user_id' not in session:
                return redirect('/login')
            if session.get('user_role') not in ['admiral', 'admin']:
                flash('Access denied: Admin privileges required', 'error')
                return redirect('/')
            return f(*args, **kwargs)
        return decorated_function

    def authenticate_user(self, username, password):
        """Authenticate user against database."""
        try:
            import psycopg2
            import psycopg2.extras

            # Database connection
            db_config = {
                'host': os.getenv('DB_HOST', 'localhost'),
                'port': os.getenv('DB_PORT', '5432'),
                'database': os.getenv('DB_NAME', 'envyro'),
                'user': os.getenv('DB_USER', 'envyro_user'),
                'password': os.getenv('DB_PASSWORD', 'envyro_pass')
            }

            conn = psycopg2.connect(**db_config)
            try:
                with conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cursor:
                    # Get user by username
                    cursor.execute("""
                        SELECT id, username, password_hash, role, is_active, failed_login_attempts, locked_until
                        FROM users WHERE username = %s
                    """, (username,))

                    user = cursor.fetchone()
                    if not user:
                        return False, "Invalid username or password"

                    # Check if account is active
                    if not user['is_active']:
                        return False, "Account is disabled"

                    # Check if account is locked
                    if user['locked_until'] and user['locked_until'] > datetime.now():
                        return False, "Account is temporarily locked due to failed login attempts"

                    # Verify password
                    if not self.security.verify_password(password, user['password_hash']):
                        # Increment failed login attempts
                        cursor.execute("SELECT increment_failed_login_attempts(%s)", (user['id'],))
                        conn.commit()
                        return False, "Invalid username or password"

                    # Reset failed login attempts on successful login
                    cursor.execute("SELECT reset_failed_login_attempts(%s)", (user['id'],))

                    # Update last login
                    cursor.execute(
                        "UPDATE users SET last_login = CURRENT_TIMESTAMP WHERE id = %s",
                        (user['id'],)
                    )

                    # Log successful login
                    cursor.execute("""
                        SELECT audit_action(%s, %s, %s, %s, %s)
                    """, (user['id'], 'login', 'auth', str(user['id']), request.remote_addr))

                    conn.commit()

                    return True, {
                        'id': user['id'],
                        'username': user['username'],
                        'role': user['role']
                    }

            finally:
                conn.close()

        except Exception as e:
            logger.error(f"Authentication error: {e}")
            return False, "Authentication service unavailable"

    def setup_routes(self):
        """Set up Flask routes with authentication."""
        # Register routes - decorators are applied to the methods
        self.app.add_url_rule('/login', 'login', self.login_page, methods=['GET', 'POST'])
        self.app.add_url_rule('/', 'dashboard', self.dashboard)
        
        # API routes with authentication
        self.app.add_url_rule('/api/services/status', 'get_service_status', self.get_service_status)
        self.app.add_url_rule('/api/services/<service_name>/<action>', 'service_action', self.service_action, methods=['POST'])
        self.app.add_url_rule('/api/services/start-all', 'start_all_services', self.start_all_services_api, methods=['POST'])
        self.app.add_url_rule('/api/services/stop-all', 'stop_all_services', self.stop_all_services_api, methods=['POST'])
        self.app.add_url_rule('/api/files/upload', 'upload_files', self.upload_files_api, methods=['POST'])
        self.app.add_url_rule('/api/files/list', 'list_files', self.list_files_api)
        self.app.add_url_rule('/api/files/clear', 'clear_files', self.clear_files_api, methods=['POST'])
        self.app.add_url_rule('/api/files/remove/<filename>', 'remove_file', self.remove_file_api, methods=['DELETE'])
        self.app.add_url_rule('/api/files/process', 'process_files', self.process_files_api, methods=['POST'])
        self.app.add_url_rule('/api/config', 'get_config', self.get_config_api)
        self.app.add_url_rule('/api/config', 'save_config', self.save_config_api, methods=['POST'])
        self.app.add_url_rule('/api/console/logs', 'get_console_logs', self.get_console_logs_api)
        self.app.add_url_rule('/api/console/clear', 'clear_console', self.clear_console_api, methods=['POST'])
        self.app.add_url_rule('/api/tests/run', 'run_tests', self.run_tests_api, methods=['POST'])

    def login_page(self):
        """Handle login page requests."""
        if request.method == 'POST':
            username = request.form.get('username')
            password = request.form.get('password')

            if not username or not password:
                flash('Username and password are required', 'error')
                return render_template_string(self.get_login_template())

            success, result = self.authenticate_user(username, password)
            if success:
                session['user_id'] = result['id']
                session['username'] = result['username']
                session['user_role'] = result['role']
                session.permanent = True
                flash(f'Welcome back, {result["username"]}!', 'success')
                return "Login successful! <a href='/'>Go to dashboard</a>"
            else:
                flash(result, 'error')
                return render_template_string(self.get_login_template())

        return render_template_string(self.get_login_template())

    def dashboard(self):
        """Handle dashboard requests."""
        if 'user_id' not in session:
            return redirect('/login')
        return render_template_string(self.get_html_template())

    def get_service_status(self):
        """Get service status (requires authentication)."""
        return jsonify({'error': 'TESTING: This should be returned'}), 403

    def service_action(self, service_name, action):
        """Control services (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        if service_name not in self.services:
            return jsonify({'error': 'Service not found'}), 404

        if action == 'start':
            self.start_service(service_name)
        elif action == 'stop':
            self.stop_service(service_name)
        elif action == 'restart':
            self.restart_service(service_name)
        else:
            return jsonify({'error': 'Invalid action'}), 400

        return jsonify({'status': 'success'})

    def start_all_services_api(self):
        """Start all services (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        self.start_all_services()
        return jsonify({'status': 'success'})

    def stop_all_services_api(self):
        """Stop all services (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        self.stop_all_services()
        return jsonify({'status': 'success'})

    def upload_files_api(self):
        """Upload files (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        if 'files' not in request.files:
            return jsonify({'error': 'No files provided'}), 400

        files = request.files.getlist('files')
        uploaded = []

        for file in files:
            if file.filename:
                filename = file.filename
                if filename not in [os.path.basename(f) for f in self.uploaded_files]:
                    # Save file to uploads directory
                    upload_dir = self.project_root / 'uploads'
                    upload_dir.mkdir(exist_ok=True)
                    file_path = upload_dir / filename
                    file.save(file_path)
                    self.uploaded_files.append(str(file_path))
                    uploaded.append(filename)
                    self.log_to_console(f"✓ Uploaded: {filename}")

        return jsonify({'uploaded': uploaded})

    def list_files_api(self):
        """List uploaded files (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        return jsonify({
            'files': [os.path.basename(f) for f in self.uploaded_files]
        })

    def clear_files_api(self):
        """Clear uploaded files (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        self.clear_uploaded_files()
        return jsonify({'status': 'success'})

    def remove_file_api(self, filename):
        """Remove a file (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        self.uploaded_files = [f for f in self.uploaded_files if os.path.basename(f) != filename]
        # Remove from disk
        upload_dir = self.project_root / 'uploads'
        file_path = upload_dir / filename
        if file_path.exists():
            file_path.unlink()
        self.log_to_console(f"✓ Removed: {filename}")
        return jsonify({'status': 'success'})

    def process_files_api(self):
        """Process files with Envyro AI (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        if not self.uploaded_files:
            return jsonify({'error': 'No files to process'}), 400

        if self.services['envyro-core']['status'] != 'running':
            return jsonify({'error': 'Envyro-Core service not running'}), 400

        self.log_to_console("🔄 Processing files with Envyro AI...")

        # Process files (placeholder for now)
        for file_path in self.uploaded_files:
            filename = os.path.basename(file_path)
            self.log_to_console(f"📄 Processing: {filename}")

        self.log_to_console("✓ File processing complete")
        return jsonify({'status': 'success'})

    def get_config_api(self):
        """Get configuration (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        return jsonify(self.env_config)

    def save_config_api(self):
        """Save configuration (requires admin)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        if session.get('user_role') not in ['admiral', 'admin']:
            return jsonify({'error': 'Admin privileges required'}), 403
        config = request.json
        if not config:
            return jsonify({'error': 'No config provided'}), 400

        env_file = self.project_root / '.env'
        try:
            with open(env_file, 'w') as f:
                f.write("# Envyro-Core Environment Configuration\n")
                f.write("# Generated by Envyro Web Launcher\n\n")
                for key, value in config.items():
                    f.write(f"{key}={value}\n")

            self.env_config = self.load_env_config()
            self.log_to_console("✓ Configuration saved successfully")
            return jsonify({'status': 'success'})
        except Exception as e:
            self.log_to_console(f"✗ Failed to save configuration: {e}")
            return jsonify({'error': str(e)}), 500

    def get_console_logs_api(self):
        """Get console logs (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        return jsonify({'logs': self.console_logs[-100:]})  # Last 100 lines

    def clear_console_api(self):
        """Clear console logs (requires authentication)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        self.console_logs.clear()
        return jsonify({'status': 'success'})

    def run_tests_api(self):
        """Run tests (requires admin)."""
        if 'user_id' not in session:
            return jsonify({'error': 'Authentication required'}), 401
        if session.get('user_role') not in ['admiral', 'admin']:
            return jsonify({'error': 'Admin privileges required'}), 403
        def run_test():
            try:
                self.log_to_console("🧪 Running Envyro test suite...")
                result = subprocess.run(
                    [sys.executable, 'comprehensive_test.py'],
                    capture_output=True, text=True, cwd=self.project_root
                )

                self.log_to_console("=== TEST OUTPUT ===")
                if result.stdout:
                    self.log_to_console(result.stdout)
                if result.stderr:
                    self.log_to_console("STDERR:")
                    self.log_to_console(result.stderr)

                if result.returncode == 0:
                    self.log_to_console("✓ All tests passed!")
                else:
                    self.log_to_console(f"✗ Tests failed with exit code {result.returncode}")

            except Exception as e:
                self.log_to_console(f"✗ Error running tests: {e}")

        threading.Thread(target=run_test, daemon=True).start()
        return jsonify({'status': 'running'})

    def get_html_template(self):
        """Get the HTML template for the web interface."""
        return """
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>🌳 Envyro Launcher - Digital Oasis Control Center</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            color: #333;
        }

        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }

        .header {
            text-align: center;
            color: white;
            margin-bottom: 30px;
            position: relative;
        }

        .header h1 {
            font-size: 2.5em;
            margin-bottom: 10px;
        }

        .header p {
            font-size: 1.2em;
            opacity: 0.9;
        }

        .user-info {
            position: absolute;
            top: 20px;
            right: 20px;
            display: flex;
            align-items: center;
            gap: 15px;
            font-size: 0.9em;
        }

        .user-info span {
            color: white;
            opacity: 0.9;
        }

        .tabs {
            display: flex;
            background: white;
            border-radius: 10px 10px 0 0;
            overflow: hidden;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
        }

        .tab-button {
            flex: 1;
            padding: 15px;
            background: #f8f9fa;
            border: none;
            cursor: pointer;
            font-size: 1em;
            font-weight: 500;
            transition: all 0.3s;
        }

        .tab-button:hover {
            background: #e9ecef;
        }

        .tab-button.active {
            background: white;
            color: #667eea;
            border-bottom: 3px solid #667eea;
        }

        .tab-content {
            background: white;
            border-radius: 0 0 10px 10px;
            padding: 30px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
            min-height: 500px;
        }

        .tab-pane {
            display: none;
        }

        .tab-pane.active {
            display: block;
        }

        .service-card {
            background: #f8f9fa;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 15px;
            border-left: 4px solid #28a745;
        }

        .service-card.stopped {
            border-left-color: #dc3545;
        }

        .service-card.starting {
            border-left-color: #ffc107;
        }

        .service-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 15px;
        }

        .service-name {
            font-size: 1.2em;
            font-weight: bold;
        }

        .status-badge {
            padding: 5px 12px;
            border-radius: 20px;
            font-size: 0.9em;
            font-weight: bold;
        }

        .status-running {
            background: #d4edda;
            color: #155724;
        }

        .status-stopped {
            background: #f8d7da;
            color: #721c24;
        }

        .status-starting {
            background: #fff3cd;
            color: #856404;
        }

        .btn-group {
            display: flex;
            gap: 10px;
        }

        .btn {
            padding: 8px 16px;
            border: none;
            border-radius: 5px;
            cursor: pointer;
            font-size: 0.9em;
            transition: all 0.3s;
        }

        .btn-primary {
            background: #667eea;
            color: white;
        }

        .btn-primary:hover {
            background: #5a67d8;
        }

        .btn-success {
            background: #28a745;
            color: white;
        }

        .btn-success:hover {
            background: #218838;
        }

        .btn-danger {
            background: #dc3545;
            color: white;
        }

        .btn-danger:hover {
            background: #c82333;
        }

        .btn-secondary {
            background: #6c757d;
            color: white;
        }

        .btn-secondary:hover {
            background: #5a6268;
        }

        .file-list {
            max-height: 300px;
            overflow-y: auto;
            border: 1px solid #dee2e6;
            border-radius: 5px;
            margin: 15px 0;
        }

        .file-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 10px;
            border-bottom: 1px solid #dee2e6;
        }

        .file-item:last-child {
            border-bottom: none;
        }

        .console {
            background: #1e1e1e;
            color: #f8f8f2;
            font-family: 'Courier New', monospace;
            padding: 15px;
            border-radius: 5px;
            height: 300px;
            overflow-y: auto;
            white-space: pre-wrap;
        }

        .form-group {
            margin-bottom: 15px;
        }

        .form-group label {
            display: block;
            margin-bottom: 5px;
            font-weight: bold;
        }

        .form-group input {
            width: 100%;
            padding: 8px;
            border: 1px solid #dee2e6;
            border-radius: 4px;
        }

        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
        }

        .upload-area {
            border: 2px dashed #dee2e6;
            border-radius: 8px;
            padding: 40px;
            text-align: center;
            transition: all 0.3s;
            cursor: pointer;
        }

        .upload-area:hover {
            border-color: #667eea;
            background: #f8f9ff;
        }

        .upload-area.dragover {
            border-color: #667eea;
            background: #f8f9ff;
        }

        @media (max-width: 768px) {
            .container {
                padding: 10px;
            }

            .header h1 {
                font-size: 2em;
            }

            .tabs {
                flex-direction: column;
            }

            .grid {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🌳 Envyro Launcher</h1>
            <p>Digital Oasis Control Center</p>
            <div class="user-info">
                <span>Welcome, {{ session.get('username', 'User') }} ({{ session.get('user_role', 'user').title() }})</span>
                <a href="{{ url_for('logout') }}" class="btn btn-outline-light btn-sm">Logout</a>
            </div>
        </div>

        <div class="tabs">
            <button class="tab-button active" onclick="showTab('services')">🚀 Services</button>
            <button class="tab-button" onclick="showTab('files')">📁 Files</button>
            <button class="tab-button" onclick="showTab('config')">⚙️ Configuration</button>
            <button class="tab-button" onclick="showTab('console')">💻 Console</button>
        </div>

        <div class="tab-content">
            <!-- Services Tab -->
            <div id="services" class="tab-pane active">
                <h2>Service Management</h2>
                <div class="btn-group" style="margin: 20px 0;">
                    <button class="btn btn-success" onclick="startAllServices()">Start All Services</button>
                    <button class="btn btn-danger" onclick="stopAllServices()">Stop All Services</button>
                    <button class="btn btn-secondary" onclick="updateServiceStatus()">Refresh Status</button>
                </div>
                <div id="services-container">
                    <!-- Services will be loaded here -->
                </div>
            </div>

            <!-- Files Tab -->
            <div id="files" class="tab-pane">
                <h2>File Management</h2>
                <div class="upload-area" id="upload-area">
                    <p>📤 Drag & drop files here or click to browse</p>
                    <input type="file" id="file-input" multiple style="display: none;">
                </div>
                <div class="btn-group" style="margin: 20px 0;">
                    <button class="btn btn-primary" onclick="processFiles()">Process with Envyro</button>
                    <button class="btn btn-danger" onclick="clearFiles()">Clear All Files</button>
                </div>
                <div class="file-list" id="file-list">
                    <!-- Files will be loaded here -->
                </div>
            </div>

            <!-- Configuration Tab -->
            <div id="config" class="tab-pane">
                <h2>Environment Configuration</h2>
                <form id="config-form">
                    <div class="grid">
                        <div class="form-group">
                            <label for="POSTGRES_DB">Database Name:</label>
                            <input type="text" id="POSTGRES_DB" name="POSTGRES_DB">
                        </div>
                        <div class="form-group">
                            <label for="POSTGRES_USER">Database User:</label>
                            <input type="text" id="POSTGRES_USER" name="POSTGRES_USER">
                        </div>
                        <div class="form-group">
                            <label for="POSTGRES_PASSWORD">Database Password:</label>
                            <input type="password" id="POSTGRES_PASSWORD" name="POSTGRES_PASSWORD">
                        </div>
                        <div class="form-group">
                            <label for="ENVYRO_VOCAB_SIZE">Vocabulary Size:</label>
                            <input type="text" id="ENVYRO_VOCAB_SIZE" name="ENVYRO_VOCAB_SIZE">
                        </div>
                        <div class="form-group">
                            <label for="ENVYRO_D_MODEL">Model Dimension:</label>
                            <input type="text" id="ENVYRO_D_MODEL" name="ENVYRO_D_MODEL">
                        </div>
                        <div class="form-group">
                            <label for="ENVYRO_N_HEADS">Number of Heads:</label>
                            <input type="text" id="ENVYRO_N_HEADS" name="ENVYRO_N_HEADS">
                        </div>
                        <div class="form-group">
                            <label for="ENVYRO_N_LAYERS">Number of Layers:</label>
                            <input type="text" id="ENVYRO_N_LAYERS" name="ENVYRO_N_LAYERS">
                        </div>
                    </div>
                    <div class="btn-group" style="margin-top: 20px;">
                        <button type="button" class="btn btn-primary" onclick="saveConfig()">Save Configuration</button>
                        <button type="button" class="btn btn-secondary" onclick="loadConfig()">Load Configuration</button>
                    </div>
                </form>
            </div>

            <!-- Console Tab -->
            <div id="console" class="tab-pane">
                <h2>System Console</h2>
                <div class="btn-group" style="margin: 20px 0;">
                    <button class="btn btn-secondary" onclick="runTests()">Run Tests</button>
                    <button class="btn btn-danger" onclick="clearConsole()">Clear Console</button>
                </div>
                <div class="console" id="console-output">
                    🌳 Envyro Launcher initialized. Welcome to the Digital Oasis!
                </div>
            </div>
        </div>
    </div>

    <script>
        let currentTab = 'services';
        let services = {};
        let uploadedFiles = [];

        // Tab switching
        function showTab(tabName) {
            document.querySelectorAll('.tab-pane').forEach(pane => pane.classList.remove('active'));
            document.querySelectorAll('.tab-button').forEach(btn => btn.classList.remove('active'));

            document.getElementById(tabName).classList.add('active');
            event.target.classList.add('active');
            currentTab = tabName;

            // Load tab data
            if (tabName === 'services') updateServiceStatus();
            if (tabName === 'files') loadFiles();
            if (tabName === 'config') loadConfig();
            if (tabName === 'console') loadConsoleLogs();
        }

        // Services management
        async function updateServiceStatus() {
            try {
                const response = await fetch('/api/services/status');
                services = await response.json();
                renderServices();
            } catch (error) {
                console.error('Error updating service status:', error);
            }
        }

        function renderServices() {
            const container = document.getElementById('services-container');
            container.innerHTML = '';

            for (const [serviceName, serviceInfo] of Object.entries(services)) {
                const card = document.createElement('div');
                card.className = `service-card ${serviceInfo.status}`;

                card.innerHTML = `
                    <div class="service-header">
                        <div class="service-name">${serviceName.charAt(0).toUpperCase() + serviceName.slice(1)}</div>
                        <div class="status-badge status-${serviceInfo.status}">${serviceInfo.status.toUpperCase()}</div>
                    </div>
                    <div class="btn-group">
                        <button class="btn btn-success" onclick="serviceAction('${serviceName}', 'start')">Start</button>
                        <button class="btn btn-danger" onclick="serviceAction('${serviceName}', 'stop')">Stop</button>
                        <button class="btn btn-secondary" onclick="serviceAction('${serviceName}', 'restart')">Restart</button>
                    </div>
                `;

                container.appendChild(card);
            }
        }

        async function serviceAction(serviceName, action) {
            try {
                const response = await fetch(`/api/services/${serviceName}/${action}`, { method: 'POST' });
                const result = await response.json();
                if (result.status === 'success') {
                    setTimeout(updateServiceStatus, 1000); // Refresh after 1 second
                }
            } catch (error) {
                console.error('Error:', error);
            }
        }

        async function startAllServices() {
            try {
                const response = await fetch('/api/services/start-all', { method: 'POST' });
                const result = await response.json();
                if (result.status === 'success') {
                    setTimeout(updateServiceStatus, 2000);
                }
            } catch (error) {
                console.error('Error:', error);
            }
        }

        async function stopAllServices() {
            try {
                const response = await fetch('/api/services/stop-all', { method: 'POST' });
                const result = await response.json();
                if (result.status === 'success') {
                    setTimeout(updateServiceStatus, 2000);
                }
            } catch (error) {
                console.error('Error:', error);
            }
        }

        // File management
        function initFileUpload() {
            const uploadArea = document.getElementById('upload-area');
            const fileInput = document.getElementById('file-input');

            uploadArea.addEventListener('click', () => fileInput.click());

            uploadArea.addEventListener('dragover', (e) => {
                e.preventDefault();
                uploadArea.classList.add('dragover');
            });

            uploadArea.addEventListener('dragleave', () => {
                uploadArea.classList.remove('dragover');
            });

            uploadArea.addEventListener('drop', (e) => {
                e.preventDefault();
                uploadArea.classList.remove('dragover');
                const files = e.dataTransfer.files;
                uploadFiles(files);
            });

            fileInput.addEventListener('change', (e) => {
                uploadFiles(e.target.files);
            });
        }

        async function uploadFiles(files) {
            const formData = new FormData();
            for (let file of files) {
                formData.append('files', file);
            }

            try {
                const response = await fetch('/api/files/upload', {
                    method: 'POST',
                    body: formData
                });
                const result = await response.json();
                if (result.uploaded) {
                    loadFiles();
                }
            } catch (error) {
                console.error('Error uploading files:', error);
            }
        }

        async function loadFiles() {
            try {
                const response = await fetch('/api/files/list');
                const result = await response.json();
                uploadedFiles = result.files;
                renderFileList();
            } catch (error) {
                console.error('Error loading files:', error);
            }
        }

        function renderFileList() {
            const fileList = document.getElementById('file-list');
            fileList.innerHTML = '';

            if (uploadedFiles.length === 0) {
                fileList.innerHTML = '<div style="text-align: center; padding: 20px; color: #6c757d;">No files uploaded yet</div>';
                return;
            }

            uploadedFiles.forEach(filename => {
                const fileItem = document.createElement('div');
                fileItem.className = 'file-item';
                fileItem.innerHTML = `
                    <span>${filename}</span>
                    <button class="btn btn-danger btn-sm" onclick="removeFile('${filename}')">Remove</button>
                `;
                fileList.appendChild(fileItem);
            });
        }

        async function removeFile(filename) {
            try {
                const response = await fetch(`/api/files/remove/${filename}`, { method: 'DELETE' });
                const result = await response.json();
                if (result.status === 'success') {
                    loadFiles();
                }
            } catch (error) {
                console.error('Error removing file:', error);
            }
        }

        async function processFiles() {
            try {
                const response = await fetch('/api/files/process', { method: 'POST' });
                const result = await response.json();
                if (result.status === 'success') {
                    loadConsoleLogs();
                }
            } catch (error) {
                console.error('Error processing files:', error);
            }
        }

        async function clearFiles() {
            try {
                const response = await fetch('/api/files/clear', { method: 'POST' });
                const result = await response.json();
                if (result.status === 'success') {
                    loadFiles();
                }
            } catch (error) {
                console.error('Error clearing files:', error);
            }
        }

        // Configuration management
        async function loadConfig() {
            try {
                const response = await fetch('/api/config');
                const config = await response.json();

                for (const [key, value] of Object.entries(config)) {
                    const input = document.getElementById(key);
                    if (input) {
                        input.value = value;
                    }
                }
            } catch (error) {
                console.error('Error loading config:', error);
            }
        }

        async function saveConfig() {
            const form = document.getElementById('config-form');
            const formData = new FormData(form);
            const config = {};

            for (let [key, value] of formData.entries()) {
                config[key] = value;
            }

            try {
                const response = await fetch('/api/config', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(config)
                });
                const result = await response.json();
                if (result.status === 'success') {
                    alert('Configuration saved successfully!');
                }
            } catch (error) {
                console.error('Error saving config:', error);
                alert('Error saving configuration');
            }
        }

        // Console management
        async function loadConsoleLogs() {
            try {
                const response = await fetch('/api/console/logs');
                const result = await response.json();
                const consoleOutput = document.getElementById('console-output');
                consoleOutput.textContent = result.logs.join('\\n');
                consoleOutput.scrollTop = consoleOutput.scrollHeight;
            } catch (error) {
                console.error('Error loading console logs:', error);
            }
        }

        async function runTests() {
            try {
                const response = await fetch('/api/tests/run', { method: 'POST' });
                const result = await response.json();
                if (result.status === 'running') {
                    setTimeout(loadConsoleLogs, 1000);
                }
            } catch (error) {
                console.error('Error running tests:', error);
            }
        }

        async function clearConsole() {
            try {
                const response = await fetch('/api/console/clear', { method: 'POST' });
                const result = await response.json();
                if (result.status === 'success') {
                    loadConsoleLogs();
                }
            } catch (error) {
                console.error('Error clearing console:', error);
            }
        }

        // Initialize
        document.addEventListener('DOMContentLoaded', function() {
            initFileUpload();
            updateServiceStatus();

            // Auto-refresh services every 5 seconds
            setInterval(updateServiceStatus, 5000);

            // Auto-refresh console logs every 2 seconds when console tab is active
            setInterval(() => {
                if (currentTab === 'console') {
                    loadConsoleLogs();
                }
            }, 2000);
        });
    </script>
</body>
</html>
        """

    def get_login_template(self):
        """Get the HTML template for the login page."""
        return """
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>🌳 Envyro Login - Digital Oasis</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .login-container {
            background: white;
            border-radius: 10px;
            padding: 40px;
            box-shadow: 0 10px 25px rgba(0, 0, 0, 0.2);
            width: 100%;
            max-width: 400px;
        }

        .login-header {
            text-align: center;
            margin-bottom: 30px;
        }

        .login-header h1 {
            color: #667eea;
            font-size: 2.5em;
            margin-bottom: 10px;
        }

        .login-header p {
            color: #666;
            font-size: 1.1em;
        }

        .form-group {
            margin-bottom: 20px;
        }

        .form-group label {
            display: block;
            margin-bottom: 5px;
            color: #333;
            font-weight: 500;
        }

        .form-group input {
            width: 100%;
            padding: 12px;
            border: 2px solid #e1e5e9;
            border-radius: 5px;
            font-size: 1em;
            transition: border-color 0.3s;
        }

        .form-group input:focus {
            outline: none;
            border-color: #667eea;
        }

        .btn {
            width: 100%;
            padding: 12px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            border-radius: 5px;
            font-size: 1em;
            font-weight: 500;
            cursor: pointer;
            transition: transform 0.2s;
        }

        .btn:hover {
            transform: translateY(-2px);
        }

        .alert {
            padding: 10px;
            border-radius: 5px;
            margin-bottom: 20px;
            font-weight: 500;
        }

        .alert-error {
            background-color: #f8d7da;
            color: #721c24;
            border: 1px solid #f5c6cb;
        }

        .alert-success {
            background-color: #d4edda;
            color: #155724;
            border: 1px solid #c3e6cb;
        }

        .alert-info {
            background-color: #d1ecf1;
            color: #0c5460;
            border: 1px solid #bee5eb;
        }
    </style>
</head>
<body>
    <div class="login-container">
        <div class="login-header">
            <h1>🌳 Envyro</h1>
            <p>Digital Oasis Control Center</p>
        </div>

        {% with messages = get_flashed_messages(with_categories=true) %}
            {% if messages %}
                {% for category, message in messages %}
                    <div class="alert alert-{{ 'error' if category == 'error' else 'success' if category == 'success' else 'info' }}">
                        {{ message }}
                    </div>
                {% endfor %}
            {% endif %}
        {% endwith %}

        <form method="POST">
            <div class="form-group">
                <label for="username">Username</label>
                <input type="text" id="username" name="username" required autofocus>
            </div>

            <div class="form-group">
                <label for="password">Password</label>
                <input type="password" id="password" name="password" required>
            </div>

            <button type="submit" class="btn">Login to Envyro</button>
        </form>
    </div>
</body>
</html>
        """

    def load_env_config(self):
        """Load environment configuration."""
        config = {}
        env_file = self.project_root / '.env'

        if env_file.exists():
            with open(env_file, 'r') as f:
                for line in f:
                    line = line.strip()
                    if line and not line.startswith('#'):
                        if '=' in line:
                            key, value = line.split('=', 1)
                            config[key] = value

        # Load defaults from .env.example if .env doesn't exist
        else:
            example_file = self.project_root / '.env.example'
            if example_file.exists():
                with open(example_file, 'r') as f:
                    for line in f:
                        line = line.strip()
                        if line and not line.startswith('#'):
                            if '=' in line:
                                key, value = line.split('=', 1)
                                config[key] = value

        return config

    def update_service_status(self):
        """Update the status of all services."""
        def check_status():
            for service_name, service_info in self.services.items():
                container_name = service_info['container']
                try:
                    result = subprocess.run(
                        ['docker', 'ps', '--filter', f'name={container_name}', '--format', '{{.Status}}'],
                        capture_output=True, text=True, cwd=self.project_root
                    )
                    if result.returncode == 0 and result.stdout.strip():
                        status = 'running'
                    else:
                        status = 'stopped'
                except Exception:
                    status = 'unknown'

                service_info['status'] = status

        threading.Thread(target=check_status, daemon=True).start()

    def start_service(self, service_name):
        """Start a specific service."""
        self.log_to_console(f"Starting {service_name} service...")
        self.services[service_name]['status'] = 'starting'

        def start():
            try:
                if service_name == 'postgres':
                    env = os.environ.copy()
                    env['POSTGRES_PASSWORD'] = self.env_config.get('POSTGRES_PASSWORD', 'envyro123')
                    result = subprocess.run(
                        ['docker-compose', 'up', '-d', 'postgres'],
                        capture_output=True, text=True, cwd=self.project_root, env=env
                    )
                else:
                    result = subprocess.run(
                        ['docker-compose', 'up', '-d', service_name],
                        capture_output=True, text=True, cwd=self.project_root
                    )

                if result.returncode == 0:
                    self.log_to_console(f"✓ {service_name} started successfully")
                    time.sleep(2)  # Wait for startup
                else:
                    self.log_to_console(f"✗ Failed to start {service_name}: {result.stderr}")
            except Exception as e:
                self.log_to_console(f"✗ Error starting {service_name}: {e}")
            finally:
                self.update_service_status()

        threading.Thread(target=start, daemon=True).start()

    def stop_service(self, service_name):
        """Stop a specific service."""
        self.log_to_console(f"Stopping {service_name} service...")

        def stop():
            try:
                result = subprocess.run(
                    ['docker-compose', 'stop', service_name],
                    capture_output=True, text=True, cwd=self.project_root
                )
                if result.returncode == 0:
                    self.log_to_console(f"✓ {service_name} stopped successfully")
                else:
                    self.log_to_console(f"✗ Failed to stop {service_name}: {result.stderr}")
            except Exception as e:
                self.log_to_console(f"✗ Error stopping {service_name}: {e}")
            finally:
                self.update_service_status()

        threading.Thread(target=stop, daemon=True).start()

    def restart_service(self, service_name):
        """Restart a specific service."""
        self.stop_service(service_name)
        time.sleep(1)
        self.start_service(service_name)

    def start_all_services(self):
        """Start all services."""
        self.log_to_console("Starting all Envyro services...")
        for service_name in self.services:
            self.start_service(service_name)

    def stop_all_services(self):
        """Stop all services."""
        self.log_to_console("Stopping all Envyro services...")
        for service_name in self.services:
            self.stop_service(service_name)

    def clear_uploaded_files(self):
        """Clear all uploaded files."""
        # Remove files from disk
        upload_dir = self.project_root / 'uploads'
        if upload_dir.exists():
            for file_path in upload_dir.glob('*'):
                if file_path.is_file():
                    file_path.unlink()
        self.uploaded_files.clear()
        self.log_to_console("✓ Cleared all uploaded files")

    def log_to_console(self, message):
        """Log a message to the console."""
        timestamp = time.strftime('%H:%M:%S')
        log_message = f"[{timestamp}] {message}"
        self.console_logs.append(log_message)
        logger.info(message)

    def run(self, host='localhost', port=5000, debug=False):
        """Run the Flask application."""
        self.log_to_console(f"🌳 Envyro Web Launcher starting on http://{host}:{port}")
        self.log_to_console("Welcome to the Digital Oasis Control Center!")

        # Open browser automatically
        try:
            webbrowser.open(f'http://{host}:{port}')
        except Exception:
            pass  # Browser might not be available

        self.app.run(host=host, port=port, debug=debug)


def main():
    """Main entry point."""
    launcher = SecureEnvyroWebLauncher()
    launcher.run()


if __name__ == "__main__":
    main()