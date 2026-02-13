"""
Custom Transformer Architecture for EnvyroAI
Implements a standard Transformer-based language model.
"""

import torch
import torch.nn as nn
import torch.nn.functional as F
import math
from typing import Optional


class PositionalEncoding(nn.Module):
    """
    Positional encoding for Transformer.
    Adds information about token positions in the sequence.
    """
    
    def __init__(self, d_model: int, max_seq_length: int = 512, dropout: float = 0.1):
        super().__init__()
        self.dropout = nn.Dropout(p=dropout)
        
        # Create positional encoding matrix
        position = torch.arange(max_seq_length).unsqueeze(1)
        div_term = torch.exp(torch.arange(0, d_model, 2) * (-math.log(10000.0) / d_model))
        
        pe = torch.zeros(max_seq_length, d_model)
        pe[:, 0::2] = torch.sin(position * div_term)
        pe[:, 1::2] = torch.cos(position * div_term)
        
        self.register_buffer('pe', pe)
    
    def forward(self, x: torch.Tensor) -> torch.Tensor:
        """
        Args:
            x: Tensor of shape [batch_size, seq_len, d_model]
        """
        x = x + self.pe[:x.size(1)]
        return self.dropout(x)


class MultiHeadAttention(nn.Module):
    """
    Multi-Head Attention mechanism.
    Core component of the Transformer architecture.
    """
    
    def __init__(self, d_model: int, n_heads: int, dropout: float = 0.1):
        super().__init__()
        assert d_model % n_heads == 0, "d_model must be divisible by n_heads"
        
        self.d_model = d_model
        self.n_heads = n_heads
        self.d_k = d_model // n_heads
        
        # Linear projections for Q, K, V
        self.q_linear = nn.Linear(d_model, d_model)
        self.k_linear = nn.Linear(d_model, d_model)
        self.v_linear = nn.Linear(d_model, d_model)
        
        # Output projection
        self.out = nn.Linear(d_model, d_model)
        
        self.dropout = nn.Dropout(dropout)
    
    def forward(
        self,
        query: torch.Tensor,
        key: torch.Tensor,
        value: torch.Tensor,
        mask: Optional[torch.Tensor] = None
    ) -> torch.Tensor:
        """
        Args:
            query: [batch_size, seq_len, d_model]
            key: [batch_size, seq_len, d_model]
            value: [batch_size, seq_len, d_model]
            mask: Optional attention mask
        """
        batch_size = query.size(0)
        
        # Linear projections and reshape for multi-head
        Q = self.q_linear(query).view(batch_size, -1, self.n_heads, self.d_k).transpose(1, 2)
        K = self.k_linear(key).view(batch_size, -1, self.n_heads, self.d_k).transpose(1, 2)
        V = self.v_linear(value).view(batch_size, -1, self.n_heads, self.d_k).transpose(1, 2)
        
        # Scaled dot-product attention
        scores = torch.matmul(Q, K.transpose(-2, -1)) / math.sqrt(self.d_k)
        
        if mask is not None:
            scores = scores.masked_fill(mask == 0, -1e9)
        
        attention = F.softmax(scores, dim=-1)
        attention = self.dropout(attention)
        
        # Apply attention to values
        x = torch.matmul(attention, V)
        
        # Concatenate heads and apply output projection
        x = x.transpose(1, 2).contiguous().view(batch_size, -1, self.d_model)
        x = self.out(x)
        
        return x


class FeedForward(nn.Module):
    """
    Position-wise Feed-Forward Network.
    Two-layer MLP with ReLU activation.
    """
    
    def __init__(self, d_model: int, d_ff: int, dropout: float = 0.1):
        super().__init__()
        self.linear1 = nn.Linear(d_model, d_ff)
        self.linear2 = nn.Linear(d_ff, d_model)
        self.dropout = nn.Dropout(dropout)
    
    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = self.linear1(x)
        x = F.relu(x)
        x = self.dropout(x)
        x = self.linear2(x)
        return x


class TransformerBlock(nn.Module):
    """
    Single Transformer encoder block.
    Consists of multi-head attention and feed-forward network with residual connections.
    """
    
    def __init__(self, d_model: int, n_heads: int, d_ff: int, dropout: float = 0.1):
        super().__init__()
        
        self.attention = MultiHeadAttention(d_model, n_heads, dropout)
        self.feed_forward = FeedForward(d_model, d_ff, dropout)
        
        self.norm1 = nn.LayerNorm(d_model)
        self.norm2 = nn.LayerNorm(d_model)
        
        self.dropout1 = nn.Dropout(dropout)
        self.dropout2 = nn.Dropout(dropout)
    
    def forward(self, x: torch.Tensor, mask: Optional[torch.Tensor] = None) -> torch.Tensor:
        # Multi-head attention with residual connection
        attn_output = self.attention(x, x, x, mask)
        x = x + self.dropout1(attn_output)
        x = self.norm1(x)
        
        # Feed-forward with residual connection
        ff_output = self.feed_forward(x)
        x = x + self.dropout2(ff_output)
        x = self.norm2(x)
        
        return x


class EnvyroTransformer(nn.Module):
    """
    Custom Transformer-based Language Model for EnvyroAI.
    
    This is the core neural network that powers EnvyroAI's generation capabilities.
    """
    
    def __init__(
        self,
        vocab_size: int,
        d_model: int = 512,
        n_heads: int = 8,
        n_layers: int = 6,
        d_ff: int = 2048,
        max_seq_length: int = 512,
        dropout: float = 0.1
    ):
        """
        Initialize the Transformer model.
        
        Args:
            vocab_size: Size of the vocabulary
            d_model: Dimension of model embeddings
            n_heads: Number of attention heads
            n_layers: Number of transformer blocks
            d_ff: Dimension of feedforward network
            max_seq_length: Maximum sequence length
            dropout: Dropout rate
        """
        super().__init__()
        
        self.d_model = d_model
        self.vocab_size = vocab_size
        
        # Token embeddings
        self.token_embedding = nn.Embedding(vocab_size, d_model)
        
        # Positional encoding
        self.pos_encoding = PositionalEncoding(d_model, max_seq_length, dropout)
        
        # Stack of Transformer blocks
        self.transformer_blocks = nn.ModuleList([
            TransformerBlock(d_model, n_heads, d_ff, dropout)
            for _ in range(n_layers)
        ])
        
        # Output projection to vocabulary
        self.output_projection = nn.Linear(d_model, vocab_size)
        
        # Dropout
        self.dropout = nn.Dropout(dropout)
        
        # Cache for causal masks
        self._mask_cache = {}
    
    def forward(
        self,
        input_ids: torch.Tensor,
        mask: Optional[torch.Tensor] = None
    ) -> torch.Tensor:
        """
        Forward pass through the model.
        
        Args:
            input_ids: Token IDs [batch_size, seq_len]
            mask: Optional attention mask
            
        Returns:
            Logits over vocabulary [batch_size, seq_len, vocab_size]
        """
        # Token embeddings with scaling
        x = self.token_embedding(input_ids) * math.sqrt(self.d_model)
        
        # Add positional encoding
        x = self.pos_encoding(x)
        
        # Pass through transformer blocks
        for block in self.transformer_blocks:
            x = block(x, mask)
        
        # Project to vocabulary
        logits = self.output_projection(x)
        
        return logits
    
    def generate_causal_mask(self, seq_len: int, device: Optional[torch.device] = None) -> torch.Tensor:
        """
        Generate a causal mask for autoregressive generation.
        Prevents attending to future tokens.
        Masks are cached to avoid redundant allocations.
        
        Args:
            seq_len: Sequence length
            device: Device to place the mask on
            
        Returns:
            Causal mask [seq_len, seq_len]
        """
        # Include device in cache key
        device_str = str(device) if device is not None else 'cpu'
        cache_key = (seq_len, device_str)
        
        # Check cache first
        if cache_key in self._mask_cache:
            return self._mask_cache[cache_key]
        
        # Create new mask
        mask = torch.tril(torch.ones(seq_len, seq_len, dtype=torch.bool))
        
        # Move to device if specified
        if device is not None:
            mask = mask.to(device)
        
        # Cache it
        self._mask_cache[cache_key] = mask
        
        return mask
