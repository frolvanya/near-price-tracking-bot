# NEAR price tracking Telegram bot
Bot is tracking current NEAR price and notifies you via Telegram without any delays when price reaches your entered threshold

@near_price_track_bot

Commands:
```
/help - display help menu

/getprice - get current NEAR price

/triggerlower {price} - notifies when NEAR price is <= current price
/triggerhigher {price} - notifies when NEAR price is >= current price
/triggers - notifies when NEAR price is <= current price

/deletelower {price} - delete trigger for lower price
/deletehigher {price} - delete trigger for higher price
/delete {price} - delete triggers for lower AND higher prices
/deleteall - delete ALL triggers
```
