
import logging
from envyro_core.envyro_ai import EnvyroAI

# Configure logging
logging.basicConfig(level=logging.INFO)

def test_ai_interactions():
    print("--- Testing EnvyroAI Interaction Updates ---")
    
    # Initialize AI without DB for prompt verification
    ai = EnvyroAI(db_config=None)
    
    # 1. Test Admiral Persona
    print("\n[Admiral Persona Test]")
    admiral_response = ai.cognitive_loop(
        "Status of the neural weights?",
        user_role="admiral"
    )
    print(f"Query: Status of the neural weights?")
    print(f"Response: {admiral_response}")
    
    # 2. Test Sprout Persona
    print("\n[Sprout Persona Test]")
    sprout_response = ai.cognitive_loop(
        "What is the Digital Oasis?",
        user_role="sprout"
    )
    print(f"Query: What is the Digital Oasis?")
    print(f"Response: {sprout_response}")
    
    # 3. Test Session History
    print("\n[Session History Test]")
    session_id = "club_member_42"
    
    # First turn
    ai.cognitive_loop(
        "Hello! I'm a new Sprout.",
        user_role="sprout",
        session_id=session_id
    )
    
    # Second turn - should include history in prompt (logged)
    print("Sending second message in session...")
    history_response = ai.cognitive_loop(
        "Can you help me grow?",
        user_role="sprout",
        session_id=session_id
    )
    
    print(f"Session history length: {len(ai.sessions[session_id])}")
    for i, msg in enumerate(ai.sessions[session_id]):
        print(f"  {i}: {msg['role']} -> {msg['content']}")
        
    # 4. Clear Session
    print("\n[Clear Session Test]")
    ai.clear_session(session_id)
    print(f"Session exists: {session_id in ai.sessions}")

if __name__ == "__main__":
    test_ai_interactions()
