#!/bin/bash
# creates a list of all packages (contains duplicates)

IFS=' '

read -ra ADDR <<< `pkg-config --static --libs libdpdk`

declare -a pkglist

for i in ${ADDR[@]}
do
	if [[ $i == *"-l:lib"* ]]
	then
		lib=`echo $i | sed 's/-l:lib//g' | sed 's/\.a//g'`
		if ! [[ ${list[*]} =~ $lib ]]
		then
			pkglist+=($lib)
		fi
	elif [[ $i == *"-lrte"* ]]
	then
		lib=(`echo $i | sed 's/-l//g'`)
		if ! [[ ${list[*]} =~ $lib ]]
		then
			pkglist+=($lib)
		fi
	fi
done

echo ${pkglist[@]}