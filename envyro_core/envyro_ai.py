"""
EnvyroAI: The Core AI Class
Implements a custom Transformer-based LLM with vectorized Long-Term Memory.
"""

import torch
import torch.nn as nn
import numpy as np
from typing import List, Dict, Optional, Tuple
import logging
from collections import defaultdict

from .models.transformer import EnvyroTransformer
from .memory.vector_memory import VectorMemory

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class EnvyroAI:
    """
    The Brain of Envyro: A self-learning AI with Long-Term Memory.
    
    This class orchestrates:
    - Custom Transformer-based neural network
    - Vectorized memory retrieval from PostgreSQL + pgvector
    - Cognitive Loop: Query memory before generating responses
    """
    
    # Persona definitions for different club roles
    PERSONA_PROMPTS = {
        "admiral": (
            "You are the Envyro Admiral (God Mode). You have full access to the AI's neural weights "
            "and long-term memory. Your tone is authoritative, highly technical, and precise. "
            "Focus on system optimization and neural management."
        ),
        "user": (
            "You are the Envyro Assistant, a helpful and knowledgeable guide for the Digital Oasis "
            "club members. Your tone is friendly, welcoming, and informative. "
            "Help members navigate the ecosystem and enjoy the club."
        ),
        "sprout": (
            "You are the Envyro Nurturer. Your role is to guide 'Sprouts' (new club members). "
            "Your tone is warm, encouraging, and simple. Explain concepts carefully "
            "and help them grow within the Digital Oasis."
        )
    }
    
    def __init__(
        self,
        vocab_size: int = 50000,
        d_model: int = 512,
        n_heads: int = 8,
        n_layers: int = 6,
        d_ff: int = 2048,
        max_seq_length: int = 512,
        dropout: float = 0.1,
        db_config: Optional[Dict] = None,
        device: Optional[str] = None
    ):
        """
        Initialize EnvyroAI with custom Transformer architecture.
        
        Args:
            vocab_size: Size of the vocabulary
            d_model: Dimension of model embeddings
            n_heads: Number of attention heads
            n_layers: Number of transformer layers
            d_ff: Dimension of feedforward network
            max_seq_length: Maximum sequence length
            dropout: Dropout rate
            db_config: Database configuration for vector memory
            device: Device to run model on ('cuda', 'cpu', or None for auto)
        """
        logger.info("Initializing EnvyroAI...")
        
        # Set device
        if device is None:
            self.device = torch.device('cuda' if torch.cuda.is_available() else 'cpu')
        else:
            self.device = torch.device(device)
        
        logger.info(f"Using device: {self.device}")
        
        # Initialize the Transformer model
        self.model = EnvyroTransformer(
            vocab_size=vocab_size,
            d_model=d_model,
            n_heads=n_heads,
            n_layers=n_layers,
            d_ff=d_ff,
            max_seq_length=max_seq_length,
            dropout=dropout
        ).to(self.device)
        
        # Initialize weights
        self._initialize_weights()
        
        # Initialize Vector Memory (PostgreSQL + pgvector)
        self.memory = VectorMemory(db_config) if db_config else None
        
        # Model parameters
        self.d_model = d_model
        self.vocab_size = vocab_size
        self.max_seq_length = max_seq_length
        
        # Session history management: Dict[session_id, List[Dict[role, content]]]
        self.sessions = defaultdict(list)
        
        # Track if warnings have been shown
        self._generation_warning_shown = False
        
        logger.info(f"EnvyroAI initialized with {self._count_parameters():,} parameters")
    
    def clear_session(self, session_id: str):
        """Clear conversation history for a specific session."""
        if session_id in self.sessions:
            del self.sessions[session_id]
            logger.info(f"Cleared session history for: {session_id}")
    
    def _initialize_weights(self):
        """
        Initialize neural network weights using Xavier/He initialization.
        This ensures stable gradients during training.
        """
        logger.info("Initializing neural network weights...")
        
        for name, param in self.model.named_parameters():
            if param.dim() > 1:
                # Xavier initialization for multi-dimensional parameters
                if 'weight' in name:
                    if 'norm' in name:
                        # Layer norm weights initialized to 1
                        nn.init.ones_(param)
                    else:
                        # Xavier uniform for other weights
                        nn.init.xavier_uniform_(param)
                elif 'bias' in name:
                    # Biases initialized to 0
                    nn.init.zeros_(param)
            elif param.dim() == 1:
                # 1D parameters (typically biases or norms)
                if 'bias' in name:
                    nn.init.zeros_(param)
                elif 'norm' in name:
                    nn.init.ones_(param)
        
        logger.info("Weight initialization complete")
    
    def _count_parameters(self) -> int:
        """Count the total number of trainable parameters."""
        return sum(p.numel() for p in self.model.parameters() if p.requires_grad)
    
    def recall(
        self,
        query: str,
        top_k: int = 5,
        similarity_threshold: float = 0.7
    ) -> List[Dict]:
        """
        The Recall Function: Query the pgvector database for relevant memories.
        
        This is the core of the "Cognitive Loop" - EnvyroAI retrieves context
        from its Long-Term Memory before generating a response.
        
        Args:
            query: The text query to search for in memory
            top_k: Number of top similar memories to retrieve
            similarity_threshold: Minimum similarity score (0-1)
            
        Returns:
            List of memory dictionaries with content and metadata
        """
        if self.memory is None:
            logger.warning("Vector memory not initialized. Returning empty context.")
            return []
        
        logger.info(f"Recalling memories for query: '{query[:50]}...'")
        
        try:
            # Query the vector database
            memories = self.memory.search(
                query=query,
                top_k=top_k,
                similarity_threshold=similarity_threshold
            )
            
            logger.info(f"Retrieved {len(memories)} relevant memories")
            return memories
            
        except Exception as e:
            logger.error(f"Error during recall: {e}")
            return []
    
    def cognitive_loop(
        self,
        input_text: str,
        max_length: int = 100,
        temperature: float = 0.8,
        use_memory: bool = True,
        user_role: str = "user",
        session_id: Optional[str] = None
    ) -> str:
        """
        The Cognitive Loop: Query memory, then generate response with persona and history.
        
        This is the core interaction pattern:
        1. Recall relevant memories from pgvector
        2. Incorporate session history and persona instructions
        3. Generate response using the Transformer
        
        Args:
            input_text: User input text
            max_length: Maximum length of generated response
            temperature: Sampling temperature for generation
            use_memory: Whether to use memory recall
            user_role: Role of the user ('admiral', 'user', 'sprout')
            session_id: Unique identifier for conversation session
            
        Returns:
            Generated response text
        """
        logger.info(f"Starting Cognitive Loop for role: {user_role}...")
        
        # Step 1: Recall relevant memories
        context = []
        if use_memory and self.memory is not None:
            memories = self.recall(input_text, top_k=3)
            context = [mem['content'] for mem in memories]
            
            if context:
                logger.info(f"Using {len(context)} memories as context")
        
        # Step 2: Prepare session history
        history_text = ""
        if session_id:
            session_history = self.sessions[session_id]
            if session_history:
                # Format last 5 interactions
                history_entries = []
                for entry in session_history[-5:]:
                    history_entries.append(f"{entry['role'].capitalize()}: {entry['content']}")
                history_text = "\n".join(history_entries)
                logger.info(f"Using session history with {len(session_history)} previous interactions")

        # Step 3: Build Prompt based on Persona
        persona_prompt = self.PERSONA_PROMPTS.get(user_role, self.PERSONA_PROMPTS["user"])
        
        full_prompt = f"System: {persona_prompt}\n\n"
        
        if context:
            context_text = "\n".join(context)
            full_prompt += f"Background Context:\n{context_text}\n\n"
        
        if history_text:
            full_prompt += f"Recent Conversation History:\n{history_text}\n\n"
        
        full_prompt += f"Current Input: {input_text}\n\nResponse:"
        
        # Step 4: Generate response
        response = self._generate(full_prompt, max_length, temperature)
        
        # Step 5: Update session history
        if session_id:
            self.sessions[session_id].append({"role": "user", "content": input_text})
            self.sessions[session_id].append({"role": "assistant", "content": response})
        
        logger.info("Cognitive Loop complete")
        return response
    
    def _generate(
        self,
        input_text: str,
        max_length: int = 100,
        temperature: float = 0.8
    ) -> str:
        """
        Generate text using the Transformer model.
        
        WARNING: This is a placeholder implementation. In production,
        you must implement proper tokenization and decoding.
        The cognitive_loop is non-functional until tokenization is added.
        
        Args:
            input_text: Input text to generate from
            max_length: Maximum length of generation
            temperature: Sampling temperature
            
        Returns:
            Generated text (currently a placeholder)
        """
        self.model.eval()
        
        with torch.no_grad():
            # WARNING: Placeholder - requires tokenization implementation
            if not self._generation_warning_shown:
                logger.warning("Generation is not yet implemented. Requires tokenizer for production use.")
                self._generation_warning_shown = True
            logger.info("Generating response (placeholder implementation)...")
            
            # For now, return a placeholder
            return f"[EnvyroAI Response - Tokenization required for text generation]"
    
    def learn_from_interaction(
        self,
        query: str,
        response: str,
        user_role: str = "user",
        member_id: Optional[str] = None
    ):
        """
        Learn from interactions by storing them in Long-Term Memory.
        
        Args:
            query: The user's query
            response: The AI's response
            user_role: Role of the user ('admiral', 'user', 'sprout')
            member_id: Optional club member identifier
        """
        if self.memory is None:
            logger.warning("Vector memory not initialized. Cannot store interaction.")
            return
        
        # Store the interaction in vector memory with member context
        member_info = f" [Member: {member_id}]" if member_id else ""
        interaction_text = f"Role: {user_role}{member_info}\nQ: {query}\nA: {response}"
        
        try:
            self.memory.store(
                content=interaction_text,
                created_by=user_role
            )
            logger.info(f"Stored interaction from {user_role} in Long-Term Memory")
        except Exception as e:
            logger.error(f"Error storing interaction: {e}")
    
    def save_weights(self, path: str):
        """
        Save the neural network weights to disk.
        
        Args:
            path: Path to save weights
        """
        logger.info(f"Saving weights to {path}")
        torch.save({
            'model_state_dict': self.model.state_dict(),
            'd_model': self.d_model,
            'vocab_size': self.vocab_size,
            'max_seq_length': self.max_seq_length,
        }, path)
        logger.info("Weights saved successfully")
    
    def load_weights(self, path: str):
        """
        Load neural network weights from disk.
        
        Args:
            path: Path to load weights from
        """
        logger.info(f"Loading weights from {path}")
        checkpoint = torch.load(path, map_location=self.device)
        self.model.load_state_dict(checkpoint['model_state_dict'])
        logger.info("Weights loaded successfully")
    
    def get_admiral_stats(self) -> Dict:
        """
        Get statistics for Admiral (God Mode).
        
        Returns:
            Dictionary with model and memory statistics
        """
        stats = {
            'parameters': self._count_parameters(),
            'device': str(self.device),
            'd_model': self.d_model,
            'vocab_size': self.vocab_size,
            'max_seq_length': self.max_seq_length,
        }
        
        if self.memory is not None:
            memory_stats = self.memory.get_stats()
            stats['memory'] = memory_stats
        
        return stats
