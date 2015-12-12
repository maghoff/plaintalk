#!/bin/bash -e

cargo doc --no-deps

s3cmd sync \
	--guess-mime-type \
	--no-mime-magic \
	--cf-invalidate \
	--delete-removed \
	target/doc/ \
	s3://magnushoff.com/rustdoc/plaintalk/
