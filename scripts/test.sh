# A script to test quickly

killall {node} &> /dev/null
rm -rf /tmp/*.db &> /dev/null
vals=(27000 27100 27200 27300)

#rand=$(gshuf -i 1000-150000000 -n 1)
TESTDIR=${TESTDIR:="testdata/hyb_$1"}
TYPE=${TYPE:="release"}

# Run the syncer now
./target/$TYPE/node \
    --config $TESTDIR/nodes-0.json \
    --ip ip_file \
    --protocol sync \
    --syncer $TESTDIR/syncer \
    --messages $2 \
    --batchsize $3 \
    --comp $4 \
    --byzantine false > logs/syncer_n_$1_$2_$3_$4.log &

for((i=0;i<$1;i++)); do
./target/$TYPE/node \
    --config $TESTDIR/nodes-$i.json \
    --ip ip_file \
    --protocol mpc \
    --syncer $TESTDIR/syncer \
    --messages $2 \
    --batchsize $3 \
    --comp $4 \
    --byzantine false > logs/party-$i-n_$1_$2_$3_$4.log &
done

# Kill all nodes sudo lsof -ti:7000-7015 | xargs kill -9
