# Gunicorn configuration file

import gc

def when_ready(server):
    """
    Called just after the server is started.
    Freeze GC before forking workers to optimize Copy-on-Write memory sharing.
    """
    gc.freeze()
    server.log.info("GC frozen for Copy-on-Write optimization")
    server.log.info(f"Objects in permanent generation: {gc.get_freeze_count()}")

def post_fork(server, worker):
    try:
        import resource
        rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        server.log.info(f"Worker {worker.pid} RSS after fork: {rss} KB")
    except ImportError:
        pass # Windows 不支持 resource 模块
