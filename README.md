# Envyro-Core 🌳

**The Brain of the Envyro AI Ecosystem**

A proprietary, self-learning AI with vectorized Long-Term Memory for the "Digital Oasis" club environment (envyro.club).

## Overview

Envyro-Core is a custom Transformer-based Language Model with an integrated Long-Term Memory system using PostgreSQL + pgvector. It implements a "Cognitive Loop" that queries its vector database for context before generating responses, enabling it to learn from every interaction.

## Features

- **Custom Transformer Architecture**: Built from scratch using PyTorch
- **Vectorized Long-Term Memory**: PostgreSQL + pgvector for semantic similarity search
- **Cognitive Loop**: AI recalls relevant memories before generating responses
- **Weight Management**: Xavier/He initialization with save/load capabilities
- **Admiral System**: God Mode access for neural weight management and knowledge pruning

## Architecture

### The Brain (EnvyroAI)
- Custom Transformer-based LLM (PyTorch/NumPy)
- Multi-head attention mechanism
- Position-wise feed-forward networks
- Configurable depth and dimensions

### The Memory (VectorMemory)
- PostgreSQL with pgvector extension
- 1536-dimensional embeddings
- Cosine similarity search
- Role-based memory attribution (Admiral, User, Sprout)

## Installation

### Prerequisites
- Python 3.8+
- PostgreSQL 12+ with pgvector extension
- CUDA (optional, for GPU acceleration)

### Quick Start

1. **Install Dependencies**
```bash
pip install -r requirements.txt
```

2. **Set Up Database**
```bash
# Create PostgreSQL database
createdb envyro

# Initialize database schema
psql -d envyro -f init_db.sql
```

3. **Configure Environment** (optional)
```bash
# Create .env file
cat > .env << EOF
ENVYRO_DB_HOST=localhost
ENVYRO_DB_PORT=5432
ENVYRO_DB_NAME=envyro
ENVYRO_DB_USER=postgres
ENVYRO_DB_PASSWORD=postgres

ENVYRO_VOCAB_SIZE=50000
ENVYRO_D_MODEL=512
ENVYRO_N_HEADS=8
ENVYRO_N_LAYERS=6
ENVYRO_D_FF=2048
ENVYRO_MAX_SEQ_LENGTH=512
ENVYRO_DROPOUT=0.1
EOF
```

## Usage

### Basic Example

```python
from envyro_core import EnvyroAI
from envyro_core.config import EnvyroConfig

# Initialize EnvyroAI with database
db_config = EnvyroConfig.get_db_config()
ai = EnvyroAI(
    vocab_size=50000,
    d_model=512,
    n_heads=8,
    n_layers=6,
    db_config=db_config
)

# The Cognitive Loop: Recall + Generate
response = ai.cognitive_loop(
    input_text="Tell me about the Digital Oasis",
    use_memory=True
)

# Learn from interaction
ai.learn_from_interaction(
    query="What is Envyro?",
    response="Envyro is a self-learning AI ecosystem...",
    user_role="admiral"
)
```

### Running the Example

```bash
python example.py
```

### Admiral Operations (God Mode)

```python
# Get neural network statistics
stats = ai.get_admiral_stats()
print(f"Total parameters: {stats['parameters']:,}")

# Save/Load neural weights
ai.save_weights("envyro_weights.pt")
ai.load_weights("envyro_weights.pt")

# Memory pruning
if ai.memory:
    ai.memory.delete_memory(memory_id=123)
    ai.memory.clear_all(confirm=True)  # Dangerous!
```

## Project Structure

```
Enviro/
├── envyro_core/           # Core AI package
│   ├── __init__.py
│   ├── envyro_ai.py       # Main EnvyroAI class
│   ├── config.py          # Configuration
│   ├── models/            # Neural network models
│   │   ├── __init__.py
│   │   └── transformer.py # Custom Transformer
│   ├── memory/            # Long-Term Memory
│   │   ├── __init__.py
│   │   └── vector_memory.py
│   └── utils/             # Utilities
├── requirements.txt       # Python dependencies
├── init_db.sql           # Database schema
├── example.py            # Usage example
└── README.md             # This file
```

## The Cognitive Loop

EnvyroAI's core interaction pattern:

1. **Recall**: Query the vector database for relevant memories
2. **Context**: Incorporate retrieved context into the prompt
3. **Generate**: Use the Transformer to generate a response
4. **Learn**: Store the interaction in Long-Term Memory

```python
# Step 1: Recall
memories = ai.recall("What is consciousness?", top_k=5)

# Step 2-4: Cognitive Loop handles it all
response = ai.cognitive_loop("What is consciousness?")
```

## The Admiral System

The Admiral has complete "God Mode" over:
- **Neural Weights**: Save, load, and prune model parameters
- **Knowledge Base**: Add, delete, and clear memories
- **User Management**: Control access privileges

Default Admiral credentials:
- Username: `admin`
- Password: `admin` (⚠️ Change in production!)

## Database Schema

### envyro_knowledge
- `id`: Serial primary key
- `content`: Text content
- `embedding`: Vector(1536) - semantic embedding
- `created_by`: Creator role (admiral/user/sprout)
- `created_at`: Timestamp

### users
- `id`: Serial primary key
- `username`: Unique username
- `password_hash`: Password hash
- `role`: User role (admiral/user/sprout)

## Configuration

Configure via environment variables or `EnvyroConfig`:

### Model Parameters
- `ENVYRO_VOCAB_SIZE`: Vocabulary size (default: 50000)
- `ENVYRO_D_MODEL`: Model dimension (default: 512)
- `ENVYRO_N_HEADS`: Number of attention heads (default: 8)
- `ENVYRO_N_LAYERS`: Number of transformer layers (default: 6)
- `ENVYRO_D_FF`: Feed-forward dimension (default: 2048)
- `ENVYRO_MAX_SEQ_LENGTH`: Maximum sequence length (default: 512)
- `ENVYRO_DROPOUT`: Dropout rate (default: 0.1)

### Database Parameters
- `ENVYRO_DB_HOST`: Database host
- `ENVYRO_DB_PORT`: Database port
- `ENVYRO_DB_NAME`: Database name
- `ENVYRO_DB_USER`: Database user
- `ENVYRO_DB_PASSWORD`: Database password

## Deployment

### Docker (Coming Soon)
```bash
docker-compose up -d
```

### VPS Deployment
1. Install PostgreSQL with pgvector
2. Install Python dependencies
3. Initialize database
4. Configure environment
5. Run FastAPI server (integration required)

## Development Roadmap

- [x] Core Transformer architecture
- [x] Weight initialization
- [x] Vector memory integration
- [x] Cognitive Loop implementation
- [ ] Tokenizer implementation
- [ ] Training pipeline
- [ ] FastAPI REST API
- [ ] React/Tailwind UI
- [ ] Rust backend integration
- [ ] Docker orchestration
- [ ] Production deployment

## Technology Stack

- **Neural Engine**: Python, PyTorch, NumPy
- **Memory**: PostgreSQL, pgvector
- **Backend**: Rust (planned), FastAPI
- **Frontend**: React, Tailwind (planned)
- **Deployment**: Docker, Nginx SSL

## Notes

- The current embedding implementation is a placeholder. In production, use proper embedding models (e.g., sentence-transformers).
- Tokenization is not yet implemented. The generate function is a placeholder.
- This is the core neural engine. API and UI integration are separate components.

## License

Proprietary - All rights reserved.

## Contact

For inquiries about the Envyro project, visit: envyro.club

---

*"Welcome to the Digital Oasis 🌳"*
