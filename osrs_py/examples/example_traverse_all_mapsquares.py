# This locates all locations (a.k.a. objects) that are named "[ph]".

# Imports the library. You must have built python wheels to make this work.
from osrs import *

# Load all location properties (e.g. their name, models and so on).
loc_configs = get_location_configs(path = "../test_data/osrs_cache")

# MapSquares implements the iterator protocol,
# so we can do for .. in .. to traverse all mapsquares.
# We could also use its .get(i,j) method
# to get the mapsquare at position i, j.
for mapsquare in MapSquares(path = "../test_data/osrs_cache"):
	try:
		locations = mapsquare.locations()
	except:
		# not all mapsquares contain locations.
		pass
	else:
		for loc in locations:
			if loc.id == 6560:
				print("hehe i am invisible")

def test():
	pass