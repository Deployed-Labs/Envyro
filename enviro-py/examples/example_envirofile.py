#!/usr/bin/env python3
"""
Example Envirofile - Web Application Stack

This demonstrates how to define a multi-container application using
pure Python instead of YAML configuration.
"""

from enviro import Container, Volume, Network, Envirofile

# Create an Envirofile for our application
app = Envirofile("web-stack")

# Define a PostgreSQL database container
db = Container(
    name="postgres",
    image="postgres:15",
    cpu=2.0,
    memory="2GB",
    env={
        "POSTGRES_USER": "appuser",
        "POSTGRES_PASSWORD": "secret",
        "POSTGRES_DB": "appdb",
    },
)

# Define a Redis cache container
cache = Container(
    name="redis",
    image="redis:7",
    cpu=0.5,
    memory="512MB",
)

# Define the web application container
# Note: Dynamic configuration based on Python logic
import os
production = os.getenv("ENV") == "production"

web = Container(
    name="web-app",
    image="myapp:latest",
    cpu=4.0 if production else 1.0,
    memory="8GB" if production else "1GB",
    env={
        "DATABASE_URL": "postgres://appuser:secret@postgres:5432/appdb",
        "REDIS_URL": "redis://redis:6379",
        "DEBUG": "false" if production else "true",
    },
    workdir="/app",
)

# In production, run multiple replicas
if production:
    web.replicas = 10

# Create a persistent volume for database data
db_volume = Volume(name="postgres-data", size="100GB")

# Create an isolated network
app_network = Network(name="app-net", cidr="10.100.0.0/24")

# Add all components to the Envirofile
app.add_container(db)
app.add_container(cache)
app.add_container(web)
app.add_volume(db_volume)
app.add_network(app_network)

# Deploy the application
if __name__ == "__main__":
    print("Deploying web stack...")
    app.deploy()
    print("\nApplication deployed successfully!")
    print("Access the web app at: http://localhost:8080")
