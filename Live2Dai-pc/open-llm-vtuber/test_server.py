import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), 'src'))
try:
    from server import app
    print("Server module imported OK")
except Exception as e:
    print(f"Import error: {e}")
    import traceback
    traceback.print_exc()
