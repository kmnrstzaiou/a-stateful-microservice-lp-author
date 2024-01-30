(function() {
  let orders = null;

  const orderEmptyTextEle = document.getElementById("order-empty-text");
  const orderTableEle = document.getElementById("order-table");
  const orderTableBodyEle = document.querySelector("#order-table tbody");

  const productIdField = document.getElementById("product-id");
  const quantityField = document.getElementById("quantity");
  const subtotalField = document.getElementById("subtotal");
  const shippingAddressField = document.getElementById("shippingAddress");
  const shippingZipField = document.getElementById("shippingZip");
  const shippingCostField = document.getElementById("shippingCost");

  function fetchOrders() {
    fetch("http://localhost:8003/orders")
      .then(r => r.json())
      .then(r => orders = r)
      .then(renderOrders)
      .catch((e) => {
        init();
      });
  }

  function init() {
    fetch("http://localhost:8003/init")
      .then(() => fetchOrders());
  }

  function renderOrders() {
    if (orders.length === 0) {
      orderEmptyTextEle.classList.remove("d-none");
      orderTableEle.classList.add("d-none");
      return;
    }

    orderEmptyTextEle.classList.add("d-none");
    orderTableEle.classList.remove("d-none");

    while (orderTableBodyEle.firstChild) {
      orderTableBodyEle.removeChild(orderTableBodyEle.firstChild);
    }

    orders.forEach((order) => {
      const row = document.createElement("tr");

      row.appendChild(createCell(order.product_id));
      row.appendChild(createCell(order.quantity));
      row.appendChild(createCell(order.subtotal));
      row.appendChild(createCell(order.shipping_address));
      row.appendChild(createCell(order.shipping_zip));
      row.appendChild(createCell(order.shipping_cost));
      row.appendChild(createCell(order.total));

      orderTableBodyEle.appendChild(row);
    });
  }

  function createCell(contents) {
    const cell = document.createElement("td");
    cell.innerText = contents;
    return cell;
  }

  function displayError(err) {
    alert("Error:" + err);
  }

  function onSaveButton() {
    const data = {
      product_id : parseFloat(productIdField.value),
      quantity : parseFloat(quantityField.value),
      subtotal : parseFloat(subtotalField.value),
      shipping_address : shippingAddressField.value,
      shipping_zip : shippingZipField.value,
      shipping_cost: parseFloat(shippingCostField.value),
      total : 0.0,
    };

    fetch("http://localhost:8003/create_order", {
      method: "POST",
      body: JSON.stringify(data),
      headers: { "Content-type": "application/json" },
    })
    .then(response => response.json())
    .then(json => { if (json.hasOwnProperty('message')) {
        alert("Error: " + json.message);
      } else {
        updateOrderForm(json)
      }
    });
  }

  function updateOrderForm(json) {
    alert("The order total for " + json.order_id + " has been updated to " + json.total);
    fetchOrders();
    document.getElementById("add-order-form").reset();
  }

  document.getElementById("save").addEventListener("click", onSaveButton);
  fetchOrders();
})();
