# A script to test quickly

killall {node} &> /dev/null
rm -rf /tmp/*.db &> /dev/null
vals=(27000 27100 27200 27300)

#rand=$(gshuf -i 1000-150000000 -n 1)
TESTDIR=${TESTDIR:="testdata/hyb_4"}
TYPE=${TYPE:="release"}

# Run the syncer now
./target/$TYPE/node \
    --config $TESTDIR/nodes-0.json \
    --ip ip_file \
    --protocol sync \
    --syncer $1 \
    --messages $2 \
    --batchsize $3 \
    --comp $4 \
    --byzantine false > logs/syncer.log &

for((i=0;i<4;i++)); do
./target/$TYPE/node \
    --config $TESTDIR/nodes-$i.json \
    --ip ip_file \
    --protocol mpc \
    --syncer $1 \
    --messages $2 \
    --batchsize $3 \
    --comp $4 \
    --byzantine false > logs/$i.log &
done

# Kill all nodes sudo lsof -ti:7000-7015 | xargs kill -9
