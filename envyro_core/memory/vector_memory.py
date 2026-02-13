"""
Vector Memory: PostgreSQL + pgvector integration for Long-Term Memory.
Stores and retrieves knowledge using semantic similarity search.
"""

import psycopg2
from psycopg2.extras import RealDictCursor
import numpy as np
from typing import List, Dict, Optional
import logging

logger = logging.getLogger(__name__)


class VectorMemory:
    """
    Long-Term Memory using PostgreSQL with pgvector extension.
    
    This class handles:
    - Storing text with vector embeddings
    - Semantic similarity search
    - Knowledge retrieval for the Cognitive Loop
    """
    
    def __init__(self, db_config: Optional[Dict] = None):
        """
        Initialize Vector Memory with database connection.
        
        Args:
            db_config: Database configuration dictionary with keys:
                - host: Database host
                - port: Database port
                - database: Database name
                - user: Database user
                - password: Database password
        """
        if db_config is None:
            # Default configuration
            logger.warning("Using default database configuration. Set ENVYRO_DB_* environment variables for production.")
            db_config = {
                'host': 'localhost',
                'port': 5432,
                'database': 'envyro',
                'user': 'postgres',
                'password': 'postgres'
            }
        
        self.db_config = db_config
        self.connection = None
        self._embedding_warning_shown = False  # Track if warning has been shown
        
        try:
            self._connect()
            logger.info("Vector Memory initialized successfully")
        except Exception as e:
            logger.error(f"Failed to initialize Vector Memory: {e}")
            logger.warning("Vector Memory will operate in degraded mode (no persistence)")
    
    def _connect(self):
        """Establish database connection."""
        self.connection = psycopg2.connect(**self.db_config)
        logger.info("Connected to PostgreSQL database")
    
    def _ensure_connection(self):
        """Ensure database connection is alive, reconnect if needed."""
        if self.connection is None or self.connection.closed:
            logger.warning("Database connection lost, reconnecting...")
            self._connect()
    
    def _text_to_embedding(self, text: str) -> np.ndarray:
        """
        Convert text to vector embedding.
        
        WARNING: This is a placeholder implementation using simple hashing.
        Semantically similar text will NOT have similar embeddings!
        In production, use a proper embedding model (e.g., sentence-transformers).
        
        Args:
            text: Input text
            
        Returns:
            Embedding vector of dimension 1536
        """
        # WARNING: Placeholder implementation - not semantically meaningful!
        if not self._embedding_warning_shown:
            logger.warning("Using placeholder hash-based embeddings. Replace with proper embedding model in production!")
            self._embedding_warning_shown = True
        
        # Placeholder: Use hash-based embedding for now
        # In production, replace with proper embedding model
        np.random.seed(hash(text) % (2**32))
        embedding = np.random.randn(1536)
        # Normalize
        embedding = embedding / np.linalg.norm(embedding)
        return embedding
    
    def store(
        self,
        content: str,
        created_by: str = 'system',
        embedding: Optional[np.ndarray] = None
    ) -> int:
        """
        Store content in Long-Term Memory.
        
        Args:
            content: Text content to store
            created_by: Creator of the content ('admiral', 'user', 'system')
            embedding: Optional pre-computed embedding vector
            
        Returns:
            ID of the stored memory
        """
        self._ensure_connection()
        
        # Generate embedding if not provided
        if embedding is None:
            embedding = self._text_to_embedding(content)
        
        try:
            with self.connection.cursor() as cursor:
                # Convert numpy array to list for PostgreSQL
                embedding_list = embedding.tolist()
                
                cursor.execute("""
                    INSERT INTO envyro_knowledge (content, embedding, created_by)
                    VALUES (%s, %s, %s)
                    RETURNING id
                """, (content, embedding_list, created_by))
                
                memory_id = cursor.fetchone()[0]
                self.connection.commit()
                
                logger.info(f"Stored memory #{memory_id} from {created_by}")
                return memory_id
                
        except Exception as e:
            self.connection.rollback()
            logger.error(f"Error storing memory: {e}")
            raise
    
    def search(
        self,
        query: str,
        top_k: int = 5,
        similarity_threshold: float = 0.7,
        query_embedding: Optional[np.ndarray] = None
    ) -> List[Dict]:
        """
        Search for similar memories using vector similarity.
        
        Args:
            query: Query text
            top_k: Number of results to return
            similarity_threshold: Minimum similarity score (0-1)
            query_embedding: Optional pre-computed query embedding
            
        Returns:
            List of memory dictionaries with content and metadata
        """
        self._ensure_connection()
        
        # Generate query embedding if not provided
        if query_embedding is None:
            query_embedding = self._text_to_embedding(query)
        
        try:
            with self.connection.cursor(cursor_factory=RealDictCursor) as cursor:
                # Convert numpy array to list for PostgreSQL
                embedding_list = query_embedding.tolist()
                
                # Use pgvector's cosine similarity operator (<=>)
                # Use CTE to compute distance once and derive similarity
                cursor.execute("""
                    WITH distances AS (
                        SELECT 
                            id,
                            content,
                            created_by,
                            created_at,
                            embedding <=> %s::vector AS distance
                        FROM envyro_knowledge
                    )
                    SELECT 
                        id,
                        content,
                        created_by,
                        created_at,
                        1 - distance AS similarity
                    FROM distances
                    WHERE 1 - distance >= %s
                    ORDER BY distance
                    LIMIT %s
                """, (embedding_list, similarity_threshold, top_k))
                
                results = cursor.fetchall()
                
                # Convert to list of dictionaries
                memories = []
                for row in results:
                    memories.append({
                        'id': row['id'],
                        'content': row['content'],
                        'created_by': row['created_by'],
                        'created_at': row['created_at'].isoformat() if row['created_at'] else None,
                        'similarity': float(row['similarity'])
                    })
                
                logger.info(f"Found {len(memories)} memories for query")
                return memories
                
        except Exception as e:
            logger.error(f"Error searching memories: {e}")
            return []
    
    def get_stats(self) -> Dict:
        """
        Get statistics about the memory database.
        
        Returns:
            Dictionary with memory statistics
        """
        self._ensure_connection()
        
        try:
            with self.connection.cursor(cursor_factory=RealDictCursor) as cursor:
                # Total memories
                cursor.execute("SELECT COUNT(*) as total FROM envyro_knowledge")
                total = cursor.fetchone()['total']
                
                # Memories by creator
                cursor.execute("""
                    SELECT created_by, COUNT(*) as count
                    FROM envyro_knowledge
                    GROUP BY created_by
                """)
                by_creator = {row['created_by']: row['count'] for row in cursor.fetchall()}
                
                return {
                    'total_memories': total,
                    'by_creator': by_creator
                }
                
        except Exception as e:
            logger.error(f"Error getting memory stats: {e}")
            return {'error': str(e)}
    
    def delete_memory(self, memory_id: int) -> bool:
        """
        Delete a memory (Admiral privilege).
        
        Args:
            memory_id: ID of memory to delete
            
        Returns:
            True if deleted successfully
        """
        self._ensure_connection()
        
        try:
            with self.connection.cursor() as cursor:
                cursor.execute("""
                    DELETE FROM envyro_knowledge
                    WHERE id = %s
                """, (memory_id,))
                
                deleted = cursor.rowcount > 0
                self.connection.commit()
                
                if deleted:
                    logger.info(f"Deleted memory #{memory_id}")
                else:
                    logger.warning(f"Memory #{memory_id} not found")
                
                return deleted
                
        except Exception as e:
            self.connection.rollback()
            logger.error(f"Error deleting memory: {e}")
            return False
    
    def clear_all(self, confirm: bool = False) -> bool:
        """
        Clear all memories (Admiral God Mode only!).
        
        Args:
            confirm: Must be True to proceed
            
        Returns:
            True if cleared successfully
        """
        if not confirm:
            logger.warning("Clear all requires confirmation")
            return False
        
        self._ensure_connection()
        
        try:
            with self.connection.cursor() as cursor:
                cursor.execute("DELETE FROM envyro_knowledge")
                deleted_count = cursor.rowcount
                self.connection.commit()
                
                logger.warning(f"CLEARED ALL MEMORIES: {deleted_count} memories deleted")
                return True
                
        except Exception as e:
            self.connection.rollback()
            logger.error(f"Error clearing memories: {e}")
            return False
    
    def close(self):
        """Close database connection."""
        if self.connection and not self.connection.closed:
            self.connection.close()
            logger.info("Vector Memory connection closed")
