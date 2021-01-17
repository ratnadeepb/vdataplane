while getopts :d:c:D:h:o:n: OPTION ; do
    case ${OPTION} in
        d) DIR=$OPTARG;;
        c) CMD=$OPTARG;;
        D) RAW_DEVICES=$OPTARG;;
        h) HUGE=$OPTARG;;
        o) ONVM=$OPTARG;;
        n) NAME=$OPTARG;;
        \?) echo "Unknown option -$OPTARG" && exit 1
    esac
done

DEVICES=()

IFS=','
for DEV in ${RAW_DEVICES} ; do
	echo $DEV
    DEVICES+=("-d $DEV:$DEV")
	echo $DEVICES
done

echo $DEVICES