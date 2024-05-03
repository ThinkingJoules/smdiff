# SMDIFF Encoder
This is mostly focusing on files less than 16mb.

This will properly diff large files, but it is a massive memory hog.

Assume about 20x whatever your two documents are: So 100mb (50+50) is about 2gb of ram to generate a patch.

This is wicked fast. Faster at generating a delta using xdelta3 or open-vcdiff (google). But I'm sure they actually care about memory usage.

Perhaps one day I can try to be more cognizant of memory usage, until then, just make sure you have some RAM.