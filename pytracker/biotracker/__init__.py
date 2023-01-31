from .shared_image import SharedImage, BufferManager
from .biotracker.biotracker import *

def get_address_and_port():
    import urllib.parse
    import os
    address = os.getenv('BIOTRACKER_COMPONENT_ADDRESS')
    assert(address is not None)
    # urlparse() and urlsplit() insists on absolute URLs starting with "//"
    result = urllib.parse.urlsplit('//' + address)
    return result.hostname, result.port
